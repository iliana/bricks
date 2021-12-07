use crate::names::{self, TeamName};
use crate::routes::player::rocket_uri_macro_player;
use crate::table::{Table, TotalsTable};
use crate::{batting, pitching, routes::ResponseResult, seasons::Season, summary};
use anyhow::Result;
use askama::Template;
use rocket::response::content::Html;
use rocket::{get, uri};
use uuid::Uuid;

#[get("/team/<id>/<sim>/<season>")]
pub fn team(id: Uuid, sim: String, season: u16) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_team(id, Season { sim, season })? {
        Some(team) => Some(Html(team.render().map_err(anyhow::Error::from)?)),
        None => None,
    })
}

fn load_team(id: Uuid, season: Season) -> Result<Option<TeamPage>> {
    let name = match names::team_name(id)? {
        Some(name) => name,
        None => return Ok(None),
    };

    let seasons = name
        .all_seasons()?
        .into_iter()
        .map(|(the_season, team_id)| {
            Ok(SeasonEntry {
                selected: if the_season == season { "selected" } else { "" },
                display: the_season.to_string(),
                path: uri!(team(
                    id = team_id,
                    sim = the_season.sim,
                    season = the_season.season,
                ))
                .to_string(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let summary = summary::team_summary(id, &season)?;
    if summary.is_empty() {
        return Ok(None);
    }

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
    seasons: Vec<SeasonEntry>,
    standard_batting: TotalsTable<{ batting::COLS + 1 }, { batting::COLS }>,
    postseason_batting: TotalsTable<{ batting::COLS + 1 }, { batting::COLS }>,
    standard_pitching: TotalsTable<{ pitching::COLS + 1 }, { pitching::COLS }>,
    postseason_pitching: TotalsTable<{ pitching::COLS + 1 }, { pitching::COLS }>,
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

struct SeasonEntry {
    path: String,
    selected: &'static str,
    display: String,
}
