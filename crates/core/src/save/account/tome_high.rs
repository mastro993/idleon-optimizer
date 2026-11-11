//! The upper half of the Tome slot list, from `js/stats/systems/w4/tome-score.js`.

use super::Decoder;
use super::fields;
use serde_json::Value;

impl Decoder<'_> {
    pub(super) fn tome_qty_high(&self, slot: usize) -> f64 {
        let ola = |index: usize| self.ola(index);
        let sum =
            |values: Vec<Option<f64>>| -> f64 { values.iter().filter_map(|value| *value).sum() };
        let max = |values: Vec<Option<f64>>| -> f64 {
            values.iter().filter_map(|value| *value).fold(0.0, f64::max)
        };
        match slot {
            64 => self.row_at("Gaming", 8),
            65 => self.array_len("Cards1") as f64,
            66 => self.row_at("Gaming", 0),
            67 => 2.0f64.powf(ola(219)),
            68 => fields::field(self.save, "FarmCrop")
                .map(|value| {
                    value
                        .as_object()
                        .map(|crops| crops.len() as f64)
                        .unwrap_or(0.0)
                })
                .unwrap_or(0.0),
            69 => sum(fields::row(self.save, "Ninja", &[104]).unwrap_or_default()),
            70 => sum(fields::row(self.save, "Summon", &[0]).unwrap_or_default()),
            71 => {
                let mut count = 0.0;
                let names = fields::strings(self.save, "Summon", &[1]).unwrap_or_default();
                for name in &names {
                    if name.starts_with("Pet")
                        || self
                            .data
                            .death_note_mob_names
                            .iter()
                            .flatten()
                            .any(|mob| mob == name)
                    {
                        count += 1.0;
                    }
                }
                count + ola(319).round()
            }
            72 => 12.0 * ola(232),
            73 => {
                let mut total = 0.0;
                let mut multi = 1.0;
                for index in 0..9 {
                    total += multi * self.nested("Summon", &[4, index]);
                    multi *= index as f64 + 3.0;
                }
                total
            }
            74 => fields::field(self.save, "Ninja")
                .and_then(|value| {
                    super::super::json::index(&value, &[102, 9])
                        .and_then(Value::as_str)
                        .map(|text| text.chars().count() as f64)
                })
                .unwrap_or(0.0),
            75 => sum(self.row_of("FamValMinigameHiscores")) + ola(99),
            76 => sum(self.row_of("PrayOwned")),
            77 => fields::row(self.save, "FarmRank", &[0])
                .map(sum)
                .unwrap_or(0.0),
            78 => ola(221),
            79 => ola(222),
            80 => sum(self.row_of("ArcadeUpg")),
            81 => self.vault(57).round(),
            82 => (65..=70)
                .map(|column| self.nested("Holes", &[11, column]))
                .sum(),
            83 => self
                .row_of_nested("Holes", &[9])
                .iter()
                .filter_map(|value| *value)
                .map(|value| super::super::formula::log(value).ceil())
                .sum(),
            84 => self
                .row_of_nested("Holes", &[1])
                .iter()
                .filter_map(|value| *value)
                .map(|value| value.max(0.0).round())
                .sum(),
            85 => ola(262),
            86 => ola(279),
            87 => self.nested("Holes", &[11, 73]),
            88 => self.nested("Holes", &[11, 74]),
            89 => self.nested("Holes", &[11, 75]),
            90 => ola(356),
            91 => self.nested("Holes", &[11, 8]),
            92 => [1, 3, 5, 7]
                .into_iter()
                .map(|column| self.nested("Holes", &[11, column]).max(0.0).round())
                .sum(),
            93 => self
                .row_of_nested("Holes", &[7])
                .iter()
                .filter_map(|value| *value)
                .map(|value| value.max(0.0).round())
                .sum(),
            94 => ola(353).min(12.0).round() + 1.0,
            95 => ola(369).round(),
            96 => (0..9)
                .map(|index| {
                    fields::object(self.save, "KRbest")
                        .and_then(|best| {
                            best.get(&format!("SummzTrz{index}"))
                                .and_then(super::super::json::number)
                        })
                        .unwrap_or(0.0)
                        .round()
                })
                .sum(),
            97 => (0..6).map(|column| self.spelunk(13, column)).sum(),
            98 => max(self.spelunk_row(1)),
            99 => sum(fields::row(self.save, "Ninja", &[103]).unwrap_or_default()),
            100 => ola(445),
            101 => ola(446),
            102 => max(self.spelunk_row(2)),
            103 => self
                .spelunk_row(5)
                .iter()
                .filter_map(|value| *value)
                .map(|value| value.max(0.0))
                .sum(),
            104 => self.spelunk_row(6).len() as f64,
            105 => self
                .characters
                .iter()
                .map(|character| character.skill(18).max(0.0))
                .fold(0.0, f64::max),
            106 => ola(443),
            107 => self.cards1_prefix("EquipmentNametag"),
            108 => self.nested("Bubba", &[1, 8]),
            109 => fields::field(self.save, "Spelunk")
                .and_then(|value| {
                    super::super::json::index(&value, &[46])
                        .and_then(|row| row.as_array().map(|items| items.len() as f64))
                })
                .unwrap_or(0.0),
            110 => self.research(7, 4).unwrap_or(0.0),
            111 => fields::row(self.save, "Research", &[11])
                .map(|row| row.len() as f64)
                .unwrap_or(0.0),
            112 => fields::row(self.save, "Research", &[9])
                .map(sum)
                .unwrap_or(0.0)
                .round(),
            113 => ola(498),
            114 => fields::row(self.save, "Research", &[0])
                .map(|row| {
                    row.iter()
                        .filter_map(|value| *value)
                        .map(|value| value.round())
                        .sum()
                })
                .unwrap_or(0.0),
            115 => fields::row(self.save, "Research", &[12])
                .map(|row| {
                    row.iter()
                        .filter_map(|value| *value)
                        .map(|value| value.max(0.0).round())
                        .sum()
                })
                .unwrap_or(0.0),
            116 => self.unique_sushi().unwrap_or(0) as f64,
            117 => ola(594),
            118 => fields::row(self.save, "RoyalG", &[0])
                .map(|row| {
                    row.iter()
                        .filter_map(|value| *value)
                        .map(|value| value.max(0.0))
                        .sum()
                })
                .unwrap_or(0.0),
            119 => self.total_statz()[4],
            120 => fields::row(self.save, "RoyalG", &[5])
                .map(|row| row.iter().filter_map(|value| *value).sum())
                .unwrap_or(0.0),
            121 => self.research(7, 9).unwrap_or(0.0),
            122 => fields::object_row(
                &fields::field(self.save, "Menagerie").unwrap_or(Value::Null),
                0,
            )
            .map(|row| fields::number_at(&row, 2))
            .unwrap_or(0.0),
            123 => fields::object_row(
                &fields::field(self.save, "Menagerie").unwrap_or(Value::Null),
                3,
            )
            .map(|row| row.iter().filter_map(|value| *value).sum())
            .unwrap_or(0.0),
            _ => 0.0,
        }
    }
}
