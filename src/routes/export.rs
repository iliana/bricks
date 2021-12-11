use crate::export::{Export, WithLeagueStats};
use crate::summary::{self, SeasonSummary};
use crate::{csv::Csv, routes::ResponseResult, seasons::Season};
use anyhow::Result;
use rocket::get;
use rocket::serde::json::Json;
use std::collections::BTreeMap;
use uuid::Uuid;

// "art is supposed to provoke strong emotions and this sure does" -- allie
macro_rules! export {
    (
        $csv_fn:ident => $csv_uri:expr,
        $json_fn:ident => $json_uri:expr,
        |$( $ident:ident: $ty:ty $(,)? )*| -> ($key:ty, $value:ty) {
            $( $body:tt )*
        }
    ) => {
        #[get($csv_uri)]
        pub fn $csv_fn(
            $( $ident: $ty , )*
        ) -> ResponseResult<Option<Csv<Vec<$value>>>> {
            let iter = { $( $body )* };
            Ok(Some(Csv(iter.map(|res| res.map(|(_, v)| v)).collect::<Result<_>>()?)))
        }

        #[get($json_uri)]
        pub fn $json_fn(
            $( $ident: $ty , )*
        ) -> ResponseResult<Option<Json<BTreeMap<$key, $value>>>> {
            let iter = { $( $body )* };
            Ok(Some(Json(iter.collect::<Result<_>>()?)))
        }
    };
}

macro_rules! season_inner {
    ($func:ident, $season:expr) => {{
        let season = $season;
        let seasons = Season::recorded()?;
        if !seasons.iter().any(|s| s == &season) {
            return Ok(None);
        }

        let summary = summary::$func(&season)?;
        let league = summary::league_totals(&season)?;
        summary.into_iter().map(move |summary| {
            Ok((
                summary.id,
                Export(WithLeagueStats {
                    inner: summary,
                    league,
                }),
            ))
        })
    }};
}

export! {
    season_player_summary_csv => "/season/<sim>/<season>/export.csv",
    season_player_summary_json => "/season/<sim>/<season>/export.json",
    |sim: String, season: u16| -> (Uuid, Export<WithLeagueStats<SeasonSummary>>) {
        season_inner!(season_player_summary, Season { sim, season })
    }
}

export! {
    season_team_summary_csv => "/season/team/<sim>/<season>/export.csv",
    season_team_summary_json => "/season/team/<sim>/<season>/export.json",
    |sim: String, season: u16| -> (Uuid, Export<WithLeagueStats<SeasonSummary>>) {
        season_inner!(season_team_summary, Season { sim, season })
    }
}
