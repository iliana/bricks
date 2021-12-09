use crate::game::{Game, Stats, Team, DEBUG_TREE, GAME_STATS_TREE};
use crate::names::box_names;
use crate::routes::player::rocket_uri_macro_player;
use crate::routes::ResponseResult;
use crate::table::{row, Table};
use crate::DB;
use anyhow::Result;
use askama::Template;
use rocket::response::content::Html;
use rocket::{get, uri};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[get("/game/<id>")]
pub fn game(id: Uuid) -> ResponseResult<Option<Html<String>>> {
    Ok(match load_game(id)? {
        GameLoad::Ok(game) => {
            let mut names = HashMap::new();
            let mut short_names = HashMap::new();
            for team in game.teams() {
                names.extend(box_names(&team.player_names, true));
                short_names.extend(box_names(&team.player_names, false));
            }

            Some(Html(
                GamePage {
                    id,
                    batters_tables: [
                        batters_table(&game.away, &names),
                        batters_table(&game.home, &names),
                    ],
                    batting_lines: [
                        batting_lines(&game.away, &short_names),
                        batting_lines(&game.home, &short_names),
                    ],
                    baserunning_lines: [
                        baserunning_lines(&game.away, &short_names),
                        baserunning_lines(&game.home, &short_names),
                    ],
                    pitchers_tables: [
                        pitchers_table(&game.away, &names),
                        pitchers_table(&game.home, &names),
                    ],
                    end_lines: end_lines(&game, &short_names),
                    game,
                }
                .render()
                .map_err(anyhow::Error::from)?,
            ))
        }
        GameLoad::Failed => Some(Html(
            GameFailedPage { id }
                .render()
                .map_err(anyhow::Error::from)?,
        )),
        GameLoad::NotFound => None,
    })
}

#[derive(Template)]
#[template(path = "game.html")]
struct GamePage {
    id: Uuid,
    game: Game,
    batters_tables: [Table<8>; 2],
    batting_lines: [Vec<Line>; 2],
    baserunning_lines: [Vec<Line>; 2],
    pitchers_tables: [Table<7>; 2],
    end_lines: Vec<Line>,
}

#[derive(Template)]
#[template(path = "failed_game.html")]
struct GameFailedPage {
    id: Uuid,
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

fn batters_table(team: &Team, names: &HashMap<Uuid, String>) -> Table<8> {
    let mut table = Table {
        header: row![
            format!("Batters \u{2013} {}", team.name.shorthand),
            "At Bats",
            "Runs Scored",
            "Hits",
            "Runs Batted In",
            "Bases on Balls (Walks)",
            "Strikeouts",
            "Left on Base",
        ],
        abbr: row!["", "AB", "R", "H", "RBI", "BB", "SO", "LOB"],
        col_class: ["w-6 xl:w-8 text-right"; 8],
        sort_method: ["none"; 8],
        rows: Vec::new(),
    };
    table.col_class[0] = "text-left";

    let mut seen = HashSet::new();
    for position in &team.lineup {
        for (i, batter) in position.iter().enumerate() {
            if let Some(stats) = team.stats.get(batter) {
                if seen.contains(batter) {
                    table.push(row![
                        names.get(batter).cloned().unwrap_or_default(),
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                    ]);
                    table.set_class(if i > 0 { "pl-4 italic" } else { "italic" });
                } else {
                    table.push(row![
                        names.get(batter).cloned().unwrap_or_default(),
                        stats.at_bats,
                        stats.runs,
                        stats.hits(),
                        stats.runs_batted_in,
                        stats.walks,
                        stats.strike_outs,
                        stats.left_on_base,
                    ]);
                    if i > 0 {
                        table.set_class("pl-4");
                    }
                    seen.insert(*batter);
                    table.set_href(0, uri!(player(id = batter)));
                }
            }
        }
    }

    table
}

fn pitchers_table(team: &Team, names: &HashMap<Uuid, String>) -> Table<7> {
    let mut table = Table {
        header: row![
            format!("Pitchers \u{2013} {}", team.name.shorthand),
            "Innings Pitched",
            "Hits Allowed",
            "Runs Allowed",
            "Bases on Balls (Walks)",
            "Strikeouts",
            "Home Runs Allowed",
        ],
        abbr: row!["", "IP", "H", "R", "BB", "SO", "HR"],
        col_class: ["w-6 xl:w-8 text-right"; 7],
        sort_method: ["none"; 7],
        rows: Vec::new(),
    };
    table.col_class[0] = "text-left";

    for pitcher in &team.pitchers {
        if let Some(stats) = team.stats.get(pitcher) {
            table.push(row![
                names.get(pitcher).cloned().unwrap_or_default(),
                stats.innings_pitched(),
                stats.hits_allowed,
                stats.earned_runs,
                stats.walks_issued,
                stats.struck_outs,
                stats.home_runs_allowed,
            ]);
            table.set_href(0, uri!(player(id = pitcher)));
        }
    }

    table
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

struct Line {
    title: &'static str,
    abbr: &'static str,
    data: String,
}

fn build_line<'a, F, S>(
    stats: impl IntoIterator<Item = (&'a Uuid, &'a Stats)>,
    names: &HashMap<Uuid, String>,
    f: F,
    force_number: bool,
) -> String
where
    F: Fn(Stats) -> S,
    S: ToString,
{
    let mut s = String::new();

    for (id, stats) in stats.into_iter().map(|v| (*v.0, *v.1)) {
        let name = names.get(&id).map(String::as_str).unwrap_or_default();
        let stat = f(stats).to_string();
        if stat.is_empty() || stat == "0" {
            // nothing
        } else {
            if !s.is_empty() {
                s.push_str("; ");
            }
            s.push_str(name);
            if force_number || stat != "1" {
                s.push('\u{a0}');
                s.push_str(&stat);
            }
        }
    }

    s
}

fn batting_lines(team: &Team, names: &HashMap<Uuid, String>) -> Vec<Line> {
    let mut lines = vec![
        Line {
            title: "Doubles",
            abbr: "2B",
            data: build_line(&team.stats, names, |s| s.doubles, false),
        },
        Line {
            title: "Triples",
            abbr: "3B",
            data: build_line(&team.stats, names, |s| s.triples, false),
        },
        Line {
            title: "Home Runs",
            abbr: "HR",
            data: build_line(&team.stats, names, |s| s.home_runs, false),
        },
        Line {
            title: "Total Bases",
            abbr: "TB",
            data: build_line(&team.stats, names, |s| s.total_bases(), true),
        },
        Line {
            title: "Sacrifice Hits",
            abbr: "SH",
            data: build_line(&team.stats, names, |s| s.sacrifice_hits, false),
        },
        Line {
            title: "Sacrifice Flies",
            abbr: "SF",
            data: build_line(&team.stats, names, |s| s.sacrifice_flies, false),
        },
        Line {
            title: "Double Plays Grounded Into",
            abbr: "GIDP",
            data: build_line(&team.stats, names, |s| s.double_plays_grounded_into, false),
        },
    ];
    lines.retain(|line| !line.data.is_empty());

    let ab_risp = team
        .stats
        .values()
        .map(|s| s.at_bats_with_risp)
        .sum::<u32>();
    if ab_risp > 0 {
        lines.push(Line {
            title: "Team Hits with Runners in Scoring Position",
            abbr: "Team RISP",
            data: format!(
                "{}-for-{}",
                team.stats.values().map(|s| s.hits_with_risp).sum::<u32>(),
                ab_risp
            ),
        });
    }

    if team.left_on_base > 0 {
        lines.push(Line {
            title: "Team Runners Left on Bases",
            abbr: "Team LOB",
            data: team.left_on_base.to_string(),
        });
    }

    lines
}

fn baserunning_lines(team: &Team, names: &HashMap<Uuid, String>) -> Vec<Line> {
    let mut lines = vec![
        Line {
            title: "Stolen Bases",
            abbr: "SB",
            data: build_line(&team.stats, names, |s| s.stolen_bases, false),
        },
        Line {
            title: "Caught Stealing",
            abbr: "CS",
            data: build_line(&team.stats, names, |s| s.caught_stealing, false),
        },
    ];
    lines.retain(|line| !line.data.is_empty());
    lines
}

fn end_lines(game: &Game, names: &HashMap<Uuid, String>) -> Vec<Line> {
    let mut lines = vec![
        Line {
            title: "Pitches-strikes",
            abbr: "",
            data: build_line(
                game.away.stats.iter().chain(&game.home.stats),
                names,
                |s| s.pitches_strikes(),
                false,
            ),
        },
        Line {
            title: "Groundouts-flyouts",
            abbr: "",
            data: build_line(
                game.away.stats.iter().chain(&game.home.stats),
                names,
                |s| s.groundouts_flyouts(),
                false,
            ),
        },
        Line {
            title: "Batters faced",
            abbr: "",
            data: build_line(
                game.away.stats.iter().chain(&game.home.stats),
                names,
                |s| s.batters_faced,
                false,
            ),
        },
    ];
    lines.retain(|line| !line.data.is_empty());
    lines
}

// =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=   =^..^=

#[allow(clippy::large_enum_variant)]
enum GameLoad {
    Ok(Game),
    Failed,
    NotFound,
}

fn load_game(id: Uuid) -> Result<GameLoad> {
    let tree = DB.open_tree(GAME_STATS_TREE)?;
    Ok(if let Some(game) = tree.get(id.as_bytes())? {
        GameLoad::Ok(serde_json::from_slice(&game)?)
    } else {
        let debug_tree = DB.open_tree(DEBUG_TREE)?;
        if debug_tree.contains_key(id.as_bytes())? {
            GameLoad::Failed
        } else {
            GameLoad::NotFound
        }
    })
}
