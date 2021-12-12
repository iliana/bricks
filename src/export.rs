use crate::game::Stats;
use crate::names::{self, TeamName};
use crate::summary::SeasonSummary;
use serde::ser::{Error, Serialize, SerializeStruct, Serializer};

pub struct WithLeagueStats<T> {
    pub inner: T,
    pub league: Stats,
}

pub struct Export<T: Exportable>(pub T);

impl<T: Exportable> Serialize for Export<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // normally we would use `serialize_map` here but the csv crate does not support it.
        // misusing `serialize_struct` here is probably ok because:
        // - serde_json: ignores `name`, only cares if `len` is 0
        // - csv: neither `name` nor `len` are used
        let mut s = serializer.serialize_struct("", 1)?;
        self.0.export(&mut s)?;
        s.end()
    }
}

pub trait Exportable {
    fn export<S>(&self, serializer: &mut S) -> Result<(), S::Error>
    where
        S: SerializeStruct;
}

impl<'a> Exportable for WithLeagueStats<Stats> {
    fn export<S>(&self, s: &mut S) -> Result<(), S::Error>
    where
        S: SerializeStruct,
    {
        macro_rules! map {
            (@inner, ? $cond:ident, $( $ident:ident, $x:expr , )*) => {{
                $(
                    s.serialize_field(
                        stringify!($ident),
                        &self.inner.$cond().then(|| $x),
                    )?;
                )*
            }};

            (@member, ? $cond:ident, $( $ident:ident $(,)? )*) => {
                map!(@inner, ?$cond, $( $ident, self.inner.$ident, )*)
            };

            (@func, ? $cond:ident, $( $ident:ident $(,)? )*) => {
                map!(@inner, ?$cond, $( $ident, self.inner.$ident(), )*)
            };

            (@func_league, ? $cond:ident, $( $ident:ident $(,)? )*) => {
                map!(@inner, ?$cond, $( $ident, self.inner.$ident(self.league), )*)
            };
        }

        s.serialize_field("is_batting", &self.inner.is_batting())?;
        map!(
            @member,
            ?is_batting,
            games_batted,
            plate_appearances,
            at_bats,
            at_bats_with_risp,
            hits_with_risp,
            singles,
            doubles,
            triples,
            home_runs,
            runs,
            runs_batted_in,
            sacrifice_hits,
            sacrifice_flies,
            stolen_bases,
            caught_stealing,
            strike_outs,
            double_plays_grounded_into,
            walks,
            left_on_base,
        );
        map!(
            @func,
            ?is_batting,
            hits,
            total_bases,
            batting_average,
            on_base_percentage,
            slugging_percentage,
            on_base_plus_slugging,
            batting_average_on_balls_in_play,
        );
        map!(@func_league, ?is_batting, ops_plus);

        s.serialize_field("is_pitching", &self.inner.is_pitching())?;
        map!(
            @member,
            ?is_pitching,
            games_pitched,
            wins,
            losses,
            games_started,
            games_finished,
            complete_games,
            shutouts,
            no_hitters,
            perfect_games,
            saves,
            batters_faced,
            outs_recorded,
            hits_allowed,
            home_runs_allowed,
            earned_runs,
            struck_outs,
            walks_issued,
            strikes_pitched,
            balls_pitched,
            flyouts_pitched,
            groundouts_pitched,
        );
        map!(
            @func,
            ?is_pitching,
            win_loss_percentage,
            earned_run_average,
            innings_pitched,
            whip,
            hits_per_9,
            home_runs_per_9,
            walks_per_9,
            struck_outs_per_9,
            struck_outs_walks_ratio,
        );
        map!(@func_league, ?is_pitching, era_plus);

        Ok(())
    }
}

impl Exportable for Option<TeamName> {
    fn export<S>(&self, s: &mut S) -> Result<(), S::Error>
    where
        S: SerializeStruct,
    {
        s.serialize_field("team_name", &self.as_ref().map(|t| &t.name))?;
        s.serialize_field("team_nickname", &self.as_ref().map(|t| &t.nickname))?;
        s.serialize_field("team_shorthand", &self.as_ref().map(|t| &t.shorthand))
    }
}

impl Exportable for WithLeagueStats<SeasonSummary> {
    fn export<S>(&self, s: &mut S) -> Result<(), S::Error>
    where
        S: SerializeStruct,
    {
        s.serialize_field("name", &self.inner.name)?;
        s.serialize_field("id", &self.inner.id)?;
        names::team_name(self.inner.team_id)
            .map_err(Error::custom)?
            .export(s)?;
        WithLeagueStats {
            inner: self.inner.stats,
            league: self.league,
        }
        .export(s)
    }
}
