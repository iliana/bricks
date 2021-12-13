// i'm so sorry - allie
#![allow(dead_code)]

use anyhow::Result;
use lazy_static::lazy_static;
use rand::{rngs::StdRng, Rng, SeedableRng};
use rand_distr::LogNormal as RngLogNormal;
use rocket::futures::stream::{self, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zerocopy::AsBytes;

use std::collections::BTreeMap;
use std::time::Instant;

use crate::chronicler::Key;
use crate::fraction::Fraction;
use crate::percentage::Pct;
use crate::table::{row, Table};
use crate::{CLIENT, DB, WEBCRISP_BASE};

type LogNormal = RngLogNormal<f64>;

pub const CACHE_TREE: &str = "salmon_cache_v1";
pub const SUMMARY_TREE: &str = "salmon_summary_v1";

lazy_static! {
    static ref SALMON_SIMULATION_QUANTITY: usize = {
        std::env::var("BRICKS_SALMON_SIMULATION_QUANTITY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(50)
    };
}

static SOULSCREAM_LETTERS: [&str; 10] = ["A", "E", "I", "O", "U", "X", "H", "A", "E", "I"];

/// Simplified configuration required for a full CRiSP simulation.
#[derive(Debug, Default, Serialize)]
pub struct SalmonConfig {
    fisheries: Vec<SalmonFishery>,
    stocks: Vec<SalmonStock>,
}

/// CRiSP salmon stock parameters.
#[derive(Debug, Default, Serialize)]
pub struct SalmonStock {
    name: &'static str,
    abbreviation: &'static str,
    hatchery_n: &'static str,
    cohort_abundance: [f64; 4],
    maturation_rate: [f64; 4],
    adult_equivalent: [f64; 4],
    maturation_by_year: Vec<[(f64, f64); 4]>,
    ev_scalars: Vec<f64>,
    log_p: [&'static str; 6],
    hatchery_flag: bool,
    msy_esc: i64,
    msh_flag: bool,
    idl: f64,
    param: f64,
    age_factor: f64,
}

/// CRiSP Fishery parameters.
#[derive(Debug, Default, Serialize)]
pub struct SalmonFishery {
    name: &'static str,
    proportions: [f64; 4],
    ocean_net: bool,
    exploitations: Vec<(&'static str, [f64; 4])>,
    policy: Vec<f64>,
    terminal: bool,
}

/// A salmonblall player; stores the minimum subset of information needed to calculate CRiSP simulation parameters and soulscream.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct SalmonblallPlayer {
    pub id: Uuid,
    pub pressurization: f64,
    pub tragicness: f64,
    pub shakespearianism: f64,
    pub ruthlessness: f64,
    pub chasiness: f64,
    pub anticapitalism: f64,
    pub moxie: f64,
    pub divinity: f64,
    pub indulgence: f64,
    pub buoyancy: f64,
    pub watchfulness: f64,
    pub soul: i32,
}

fn stat_soulscream(s: f64, j: f64) -> &'static str {
    if j * 10.0 == 0.0 {
        "undefined"
    } else {
        SOULSCREAM_LETTERS[((s % j) / j * 10.0).floor() as usize]
    }
}

impl SalmonblallPlayer {
    /// Generates a player's soulscream. Based on the blaseball_mike implementation.
    pub fn soulscream(&self, byte_limit: Option<usize>) -> String {
        let mut s = String::new();
        let byte_limit = byte_limit.unwrap_or(usize::MAX);

        for i in 0..self.soul {
            if s.len() >= byte_limit {
                return s;
            }

            let j = 10f64.powi(-i);
            let sub_scream = [
                stat_soulscream(self.pressurization, j),
                stat_soulscream(self.divinity, j),
                stat_soulscream(self.tragicness, j),
                stat_soulscream(self.shakespearianism, j),
                stat_soulscream(self.ruthlessness, j),
            ]
            .concat();
            s.push_str(&sub_scream);
            s.push_str(&sub_scream);
            s.push_str(&sub_scream[0..=0]);
        }

        s
    }

    /// Generates n CRiSP simulation parameters, using player stats and an rng seeded by the player's soulscream.
    pub fn generate_n_simulations(&self, n: usize) -> Vec<SalmonConfig> {
        let seed: [u8; 32] = {
            let s = self.soulscream(Some(32));
            if s.len() < 32 {
                let remainder = 32 - s.len();
                s.into_bytes()
                    .into_iter()
                    .chain(std::iter::repeat(0).take(remainder))
                    .collect::<Vec<u8>>()
                    .try_into()
                    .unwrap()
            } else {
                s.into_bytes()
                    .drain(0..32)
                    .collect::<Vec<u8>>()
                    .try_into()
                    .unwrap()
            }
        };

        let mut rng = StdRng::from_seed(seed);
        std::iter::repeat_with(|| self.generate_crisp_parameters(&mut rng))
            .take(n)
            .collect()
    }

    /// Generates CRiSP simulation parameters using player stats and provided rng. Based on https://github.com/alisww/yuuko/blob/main/yuuko/clockwork/salmon.py
    pub fn generate_crisp_parameters(&self, rng: &mut impl Rng) -> SalmonConfig {
        let cohort_abundance: [f64; 4] = {
            let base = (self.buoyancy / 2.0 + self.pressurization / 2.0) * 10f64.powi(6);
            [
                base,
                base / (rng.gen_range(0.0..1.0) + 1.0),
                base / (rng.gen_range(0.0..1.0) + 3.0),
                base / (rng.gen_range(0.0..1.0) + 15.0),
            ]
        };

        let maturation_rate: [f64; 4] = [
            self.chasiness / rng.gen_range(0.0..1.0),
            self.chasiness * rng.gen_range(0.0..1.0) + 0.2,
            self.chasiness * (rng.gen_range(0.0..1.0) + 2.0),
            0.99999999,
        ];

        let adult_equivalent: [f64; 4] = [
            rng.sample(LogNormal::new(-self.indulgence, 0.2).unwrap()),
            rng.sample(LogNormal::new(-self.indulgence + 0.1, 0.2).unwrap()),
            rng.sample(LogNormal::new(-self.indulgence + 0.3, 0.2).unwrap()),
            0.99999999,
        ];

        let exploit_rate_distr = LogNormal::new(-self.anticapitalism, 0.4).unwrap();
        let exploit_rates: [f64; 4] = [
            rng.sample(exploit_rate_distr),
            rng.sample(exploit_rate_distr),
            rng.sample(exploit_rate_distr),
            rng.sample(exploit_rate_distr),
        ];

        let param: f64 = rng.sample(LogNormal::new(-self.moxie + 1.6, 0.3).unwrap());
        let age_factor: f64 = rng.sample(LogNormal::new(-self.divinity + 1.6, 0.3).unwrap());

        let ev_scalar_distr = LogNormal::new(self.indulgence, 1.0916).unwrap();
        let ev_scalars: Vec<f64> = std::iter::repeat_with(|| rng.sample(ev_scalar_distr))
            .take(39)
            .collect();

        let proportions: [f64; 4] = [
            rng.sample(LogNormal::new(-(self.watchfulness * 2.0), 0.3).unwrap()),
            rng.sample(LogNormal::new(-(self.watchfulness * 2.0), 0.3).unwrap()),
            rng.sample(LogNormal::new(-((self.watchfulness * 2.0).powi(-5)), 0.3).unwrap()),
            rng.sample(LogNormal::new(-((self.watchfulness * 2.0).powi(-5)), 0.3).unwrap()),
        ];

        SalmonConfig {
            stocks: vec![SalmonStock {
                name: "Salmon Institute T",
                abbreviation: "SIBR",
                hatchery_n: "Where the Salmon Are",
                cohort_abundance,
                maturation_rate,
                adult_equivalent,
                maturation_by_year: std::iter::repeat([
                    (maturation_rate[0], adult_equivalent[0]),
                    (maturation_rate[1], adult_equivalent[1]),
                    (maturation_rate[2], adult_equivalent[2]),
                    (maturation_rate[3], adult_equivalent[3]),
                ])
                .take(39)
                .collect(),
                ev_scalars,
                log_p: ["Log", "Normal", "Indep", "-0.6343", "1.0916", "911"],
                hatchery_flag: true,
                msy_esc: 7000,
                msh_flag: true,
                idl: 1.0,
                param,
                age_factor,
            }],
            fisheries: vec![SalmonFishery {
                name: "Fishy T",
                proportions,
                ocean_net: false,
                exploitations: std::iter::repeat(("SIBR", exploit_rates))
                    .take(39)
                    .collect(),
                policy: std::iter::repeat(1.0).take(39).collect(),
                terminal: true,
            }],
        }
    }
}

// ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<>

// so the webcrisp api has some weird data modeling from allie from a year ago and crisp from way more than a year ago.
// this struct helps smooth over that by making the raw sim results into a saner data structure
#[derive(Debug, Deserialize)]
struct RawSimResults {
    catch: BTreeMap<String, BTreeMap<String, i64>>,
    abundances: BTreeMap<String, Vec<[String; 2]>>,
    stocks: BTreeMap<String, BTreeMap<String, Vec<[i64; 2]>>>,
}

impl RawSimResults {
    fn process(self) -> Result<BTreeMap<String, FisheryResults>> {
        let mut res: BTreeMap<String, FisheryResults> = BTreeMap::new();
        for (fishery, catches_by_year) in self.catch {
            let entry = res.entry(fishery.to_owned()).or_default();
            for (year, val) in catches_by_year {
                entry.catches.push((year.parse::<i64>()?, val));
            }
            entry.catches.sort_by(|a, b| a.0.cmp(&b.0));
        }

        for (fishery, abundances_by_year) in self.abundances {
            let entry = res.entry(fishery.to_owned()).or_default();
            for v in abundances_by_year {
                let (year, val) = (&v[0], &v[1]);

                entry
                    .abundances
                    .push((year.parse::<i64>()?, val.parse::<i64>()?));
            }

            entry.abundances.sort_by(|a, b| a.0.cmp(&b.0));
        }

        for (stock, fisheries) in self.stocks {
            for (fishery, vals) in fisheries {
                res.entry(fishery.to_owned()).or_default().stocks.insert(
                    stock.to_owned(),
                    vals.into_iter().map(|v| (v[0], v[1])).collect(),
                );
            }
        }

        Ok(res)
    }
}

// just to make things clear
type Year = i64;

/// CRiSP results for a fishery.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FisheryResults {
    pub catches: Vec<(Year, i64)>, // year : catches
    pub abundances: Vec<(Year, i64)>,
    pub stocks: BTreeMap<String, Vec<(Year, i64)>>,
}

pub async fn process_players(players: Vec<(Key, SalmonblallPlayer)>) -> Result<()> {
    let summary_tree = DB.open_tree(SUMMARY_TREE)?;
    let cache_tree = DB.open_tree(CACHE_TREE)?;

    stream::iter(players)
        .map(Ok)
        .try_for_each_concurrent(25, |(k, v)| {
            process_single_player(k, v, &cache_tree, &summary_tree)
        })
        .await
}

async fn process_single_player(
    key: Key,
    player: SalmonblallPlayer,
    cache_tree: &sled::Tree,
    summary_tree: &sled::Tree,
) -> Result<()> {
    if !summary_tree.contains_key(key.as_bytes())? {
        let start = Instant::now();

        let results = if let Some(ref cached) = cache_tree.get(key.as_bytes())? {
            serde_json::from_slice(cached)?
        } else {
            let parameters = player.generate_n_simulations(*SALMON_SIMULATION_QUANTITY);

            let response = CLIENT
                .post(format!("{}/batch_simple_sim", WEBCRISP_BASE))
                .json(&parameters)
                .send()
                .await?
                .json::<Vec<RawSimResults>>()
                .await?
                .into_iter()
                .map(|sim| sim.process().map(|mut t| t.remove("Fishy T").unwrap()))
                .collect::<Result<Vec<FisheryResults>>>()?;

            cache_tree.insert(key.as_bytes(), serde_json::to_vec(&response)?)?;

            response
        };

        let successful = results.iter().fold(0, |acc, sim| {
            if let Some(v) = sim.stocks.get("SIB").and_then(|r| r.last()) {
                if v.1 > 0 {
                    return acc + 1;
                }
            }

            acc
        });

        let (years, decrease) = results
            .iter()
            .flat_map(|sim| {
                sim.stocks["SIB"]
                    .windows(2)
                    .take_while(|v| v[0].1 > 0 && v[1].1 > 0)
                    .map(|v| Fraction::new(v[1].1, v[0].1 as u64).round())
                    .collect::<Vec<i64>>()
            })
            .fold((0, 0), |(l, acc), x| (l + 1, acc + x));

        let avg_decrease = Fraction::new(decrease, years) * 100.into();

        let mut times_until_extinction = results
            .iter()
            .map(|sim| sim.stocks["SIB"].iter().take_while(|(_, v)| v > &0).count() as i64)
            .collect::<Vec<i64>>();
        times_until_extinction.sort_unstable();

        summary_tree.insert(
            key.as_bytes(),
            serde_json::to_vec(&SalmonblallSummary {
                successful,
                total: results.len(),
                failed: results.len() - successful,
                avg_decrease_nom: avg_decrease.numer,
                avg_decrease_denom: avg_decrease.denom,
                time_until_extinction: times_until_extinction[times_until_extinction.len() / 2],
            })?,
        )?;

        log::info!(
            "processed CRiSP simulations for {} in {:?}",
            player.id,
            start.elapsed()
        );
    }

    Ok(())
}

// ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<>

#[derive(Serialize, Deserialize)]
pub struct SalmonblallSummary {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub avg_decrease_nom: i64,
    pub avg_decrease_denom: u64,
    pub time_until_extinction: i64,
}

pub async fn pick_best_result(all: Vec<FisheryResults>) -> Option<FisheryResults> {
    let mut best_score = i64::MIN;
    let mut best_sim = None;

    for sim in all {
        let mut score: i64 = 0;
        if sim.abundances.last().map(|(_, v)| *v).unwrap_or(0) > 0 {
            score += 1_000_000;
        }

        score += sim.stocks["SIB"]
            .windows(2)
            .map(|w| w[0].1 - w[1].1)
            .sum::<i64>()
            / sim.stocks["SIB"].len() as i64;

        if score >= best_score {
            best_score = score;
            best_sim = Some(sim);
        }
    }

    best_sim
}

pub const COLS: usize = 3;

pub fn table(salmon: SalmonblallSummary) -> Table<COLS> {
    let mut table = Table::new(
        [
            ("Median Time Until Extinction", "MTUE"),
            ("Failures in Salmon Harvesting", "FISH%"),
            (
                "Average year-by-year decrease in salmon populations",
                "ADSP%",
            ),
        ],
        "text-right",
        "number",
    );

    table.push(row![
        format!("{} yrs", salmon.time_until_extinction),
        Pct::<2>(Fraction::new(salmon.failed as i64, salmon.total as u64) * 100.into()),
        Pct::<2>(Fraction::new(
            salmon.avg_decrease_nom,
            salmon.avg_decrease_denom
        ))
    ]);

    table
}

// ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<> ::<>

#[cfg(test)]
mod tests {
    use super::SalmonblallPlayer;

    #[test]
    fn test_soulscream() {
        // https://onomancer.sibr.dev/api/generateStats2?name=Hatsune%20Miku
        let miku = SalmonblallPlayer {
            pressurization: 0.05102526565489107,
            divinity: 0.49260097923896856,
            tragicness: 0.11459214093935316,
            shakespearianism: 0.9264926244757927,
            ruthlessness: 0.5720582688050835,
            chasiness: 0.7760507277652053,
            indulgence: 1.0279934617333082,
            anticapitalism: 0.7794113941162841,
            moxie: 0.6535505979327512,
            buoyancy: 0.8212275560201918,
            watchfulness: 0.6983992521888471,
            soul: 9,
        };

        assert_eq!(miku.soulscream(), String::from("AUEIXAUEIXAXIEIAXIEIAXEIUHIEIUHIEAHXUAAHXUAAIAIIXIAIIXIXAIIEXAIIEXIIEHIIIEHIIHAUIHHAUIHHXIAUEXIAUEX"));

        // KCBM's Benson Yolk - https://api.sibr.dev/chronicler/v2/entities?type=player&id=82bf8959-480e-435b-9b26-b4738ca141c8&at=2021-12-08T07:43:07.194766Z
        let benson = SalmonblallPlayer {
            pressurization: 0.9428976609030266,
            divinity: 0.390254105871104,
            tragicness: 0.2787331632251331,
            shakespearianism: 0.6689638107118578,
            ruthlessness: 0.9629654468152646,
            chasiness: 0.5749967575767938,
            indulgence: 0.8092254212944967,
            anticapitalism: 0.7456802346202227,
            moxie: 0.4521547875598889,
            buoyancy: 0.86486591279949,
            watchfulness: 0.42204830723953896,
            soul: 6,
        };

        assert_eq!(
            benson.soulscream(),
            String::from("IOIHIIOIHIIUIAHHUIAHHUIAEEIIAEEIIEIAIIEIAIIEIXOHHIXOHHIAUOOXAUOOXA")
        );
    }
}
