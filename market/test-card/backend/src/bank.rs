//! 离线银行卡 BIN 参考表。这里是固定版本开源数据中的银联信用卡号段，
//! 不是银行签发信息、支付平台沙箱卡或实时的发卡行查询服务。

use serde::{Deserialize, Serialize};

use crate::brand::Brand;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Bank {
    Cmb,
    Ccb,
    Icbc,
    Abc,
    Boc,
    Bocom,
    Psbc,
    Cib,
    Spdb,
    Citic,
    Cmbc,
    Ceb,
    Pab,
    Cgb,
    Hxb,
    Bob,
    Bosh,
    Bon,
}

pub const ALL: [Bank; 18] = [
    Bank::Cmb,
    Bank::Ccb,
    Bank::Icbc,
    Bank::Abc,
    Bank::Boc,
    Bank::Bocom,
    Bank::Psbc,
    Bank::Cib,
    Bank::Spdb,
    Bank::Citic,
    Bank::Cmbc,
    Bank::Ceb,
    Bank::Pab,
    Bank::Cgb,
    Bank::Hxb,
    Bank::Bob,
    Bank::Bosh,
    Bank::Bon,
];

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Prefix {
    pub bin: &'static str,
    pub length: usize,
}

const fn credit(bin: &'static str) -> Prefix {
    Prefix { bin, length: 16 }
}

impl Bank {
    pub fn id(self) -> &'static str {
        match self {
            Self::Cmb => "cmb",
            Self::Ccb => "ccb",
            Self::Icbc => "icbc",
            Self::Abc => "abc",
            Self::Boc => "boc",
            Self::Bocom => "bocom",
            Self::Psbc => "psbc",
            Self::Cib => "cib",
            Self::Spdb => "spdb",
            Self::Citic => "citic",
            Self::Cmbc => "cmbc",
            Self::Ceb => "ceb",
            Self::Pab => "pab",
            Self::Cgb => "cgb",
            Self::Hxb => "hxb",
            Self::Bob => "bob",
            Self::Bosh => "bosh",
            Self::Bon => "bon",
        }
    }

    /// Curated from the pinned CC/16 rows documented in data/README.md.
    /// All retained records start with 62 and match the second source exactly.
    pub fn prefixes(self) -> &'static [Prefix] {
        match self {
            Self::Cmb => &const { [credit("622575"), credit("622576"), credit("622577")] },
            Self::Ccb => &const { [credit("622168"), credit("622708"), credit("625966")] },
            Self::Icbc => &const { [credit("622210"), credit("622211"), credit("622212")] },
            Self::Abc => &const { [credit("622836"), credit("622837"), credit("622838")] },
            Self::Boc => &const { [credit("622760"), credit("622761"), credit("622762")] },
            Self::Bocom => &const { [credit("622251"), credit("622252"), credit("622253")] },
            Self::Psbc => &const { [credit("622810"), credit("622811"), credit("622812")] },
            Self::Cib => &const { [credit("622571"), credit("622572"), credit("622573")] },
            Self::Spdb => &const { [credit("622176"), credit("622177"), credit("622228")] },
            Self::Citic => &const { [credit("622453"), credit("622456"), credit("622678")] },
            Self::Cmbc => &const { [credit("622600"), credit("622601"), credit("622602")] },
            Self::Ceb => &const { [credit("622161"), credit("622570"), credit("622650")] },
            Self::Pab => &const { [credit("622155"), credit("622156"), credit("622157")] },
            Self::Cgb => &const { [credit("622555"), credit("622556"), credit("622557")] },
            Self::Hxb => &const { [credit("622636"), credit("622637"), credit("622638")] },
            Self::Bob => &const { [credit("622163"), credit("622851"), credit("622852")] },
            Self::Bosh => &const { [credit("622148"), credit("622149"), credit("622268")] },
            Self::Bon => &const { [credit("622282"), credit("622318"), credit("622778")] },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Definition {
    pub id: Bank,
    pub brand: Brand,
    pub card_type: &'static str,
    pub prefixes: &'static [Prefix],
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub url: &'static str,
    pub revision: &'static str,
    pub data_path: &'static str,
    pub license: &'static str,
    pub license_text: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    #[serde(flatten)]
    pub reference: Reference,
    pub checked_at: &'static str,
    pub compared_with: Reference,
}

pub const SOURCE: Source = Source {
    reference: Reference {
        url: "https://github.com/hexindai/bcbc",
        revision: "de631827ffe8db2792d140f3476b02498fc1244f",
        data_path: "data/bin.csv",
        license: "MIT",
        license_text: include_str!("../data/bcbc-LICENSE"),
    },
    checked_at: "2026-10-16",
    compared_with: Reference {
        url: "https://github.com/caijf/bankcard",
        revision: "79af3e5d1bd381537c2f0e95c03584c911963d1e",
        data_path: "src/data/bin.ts",
        license: "MIT",
        license_text: include_str!("../data/bankcard-LICENSE"),
    },
};

#[derive(Debug, Serialize)]
pub struct Catalog {
    pub banks: Vec<Definition>,
    pub source: Source,
}

pub fn catalog() -> Catalog {
    Catalog {
        banks: ALL
            .into_iter()
            .map(|id| Definition {
                id,
                brand: Brand::Unionpay,
                card_type: "credit",
                prefixes: id.prefixes(),
            })
            .collect(),
        source: SOURCE,
    }
}

/// Only identifies the reference row: no issuance or card-status assertion.
pub fn detect(digits: &str) -> Option<(Bank, &'static str)> {
    if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    ALL.into_iter().find_map(|bank| {
        bank.prefixes()
            .iter()
            .find(|prefix| digits.len() == prefix.length && digits.starts_with(prefix.bin))
            .map(|prefix| (bank, prefix.bin))
    })
}

#[cfg(test)]
#[path = "bank_test.rs"]
mod tests;
