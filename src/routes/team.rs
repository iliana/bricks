use crate::names::{self, TeamName};
use crate::routes::player::rocket_uri_macro_player;
use crate::table::{Table, TotalsTable};
use crate::{batting, pitching, routes::ResponseResult, seasons, summary, DB};
use anyhow::{ensure, Result};
use askama::Template;
use rocket::response::content::Html;
use rocket::{get, uri};
use uuid::Uuid;

#[get("/team/<id>/<sim>/<season>")]
pub fn team(id: Uuid, sim: &str, season: u16) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_team(id, sim, season)? {
        Some(team) => Some(Html(team.render().map_err(anyhow::Error::from)?)),
        None => None,
    })
}

fn load_team(id: Uuid, sim: &str, season: u16) -> Result<Option<TeamPage>> {
    let name = match names::team_name(id)? {
        Some(name) => name,
        None => return Ok(None),
    };

    let common_names_tree = DB.open_tree(names::COMMON_TREE)?;
    let mut seasons = Vec::new();
    for row in common_names_tree.scan_prefix(&name.emoji_hash().to_ne_bytes()) {
        const SEASON_START: usize = std::mem::size_of::<u64>();
        const SIM_START: usize = SEASON_START + std::mem::size_of::<u16>();

        let (key, value) = row?;
        ensure!(key.len() >= SIM_START, "invalid key in common names tree");
        let the_sim = std::str::from_utf8(&key[SIM_START..])?;
        let mut season_bytes = [0; std::mem::size_of::<u16>()];
        season_bytes.copy_from_slice(&key[SEASON_START..SIM_START]);
        let the_season = u16::from_ne_bytes(season_bytes);
        seasons.push(Season {
            path: uri!(team(
                id = Uuid::from_slice(&value)?,
                sim = the_sim,
                season = the_season
            ))
            .to_string(),
            selected: if sim == the_sim && season == the_season {
                "selected"
            } else {
                ""
            },
            display: format!(
                "{}, Season {}",
                seasons::era_name(the_sim, the_season)?.unwrap_or_else(|| the_sim.to_owned()),
                the_season + 1
            ),
        })
    }

    let mut summary = summary::team_summary(id, sim, season)?.collect::<Result<Vec<_>>>()?;
    if summary.is_empty() {
        return Ok(None);
    }
    summary.sort_unstable();

    macro_rules! tabler {
        ($tabler:expr, $filter:expr) => {{
            let mut ident_table = Table::new([("Player", "")], "text-left", "none");
            for row in summary.iter().filter($filter) {
                let player = names::player_name(row.player_id)?.unwrap_or_default();
                ident_table.push([player]);
                ident_table.set_href(0, uri!(player(id = row.player_id)));
            }
            let stats_table = $tabler(summary.iter().filter($filter).map(|row| row.stats));
            TotalsTable {
                table: stats_table.table.insert(0, ident_table),
                totals: stats_table.totals,
            }
        }};
    }

    Ok(Some(TeamPage {
        team: name,
        seasons,
        standard_batting: tabler!(batting::table, |s| !s.is_postseason && s.stats.is_batting()),
        postseason_batting: tabler!(batting::table, |s| s.is_postseason && s.stats.is_batting()),
        standard_pitching: tabler!(pitching::table, |s| !s.is_postseason
            && s.stats.is_pitching()),
        postseason_pitching: tabler!(pitching::table, |s| s.is_postseason
            && s.stats.is_pitching()),
    }))
}

#[derive(Template)]
#[template(path = "team.html")]
struct TeamPage {
    team: TeamName,
    seasons: Vec<Season>,
    standard_batting: TotalsTable<{ batting::COLS + 1 }, { batting::COLS }>,
    postseason_batting: TotalsTable<{ batting::COLS + 1 }, { batting::COLS }>,
    standard_pitching: TotalsTable<{ pitching::COLS + 1 }, { pitching::COLS }>,
    postseason_pitching: TotalsTable<{ pitching::COLS + 1 }, { pitching::COLS }>,
}

struct Season {
    path: String,
    selected: &'static str,
    display: String,
}
