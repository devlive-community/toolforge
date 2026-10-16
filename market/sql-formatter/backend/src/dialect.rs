//! 方言差异：词法细节、需要独立成行的子句短语与额外关键字。
//!
//! 格式化引擎只区分通用 / PostgreSQL / SQL Server 三种词法；其余方言选用最接近的一种，
//! 引擎不认识的写法（三引号与原始字符串、Oracle q'' 字符串、SQLite 方括号标识符、
//! BigQuery ARRAY<...> 类型等）在格式化前替换为占位符、之后原样还原。

use serde::Deserialize;
use sqlformat::Dialect;

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DialectKind {
    #[default]
    Generic,
    Postgresql,
    Sqlserver,
    Mysql,
    Sqlite,
    Oracle,
    Bigquery,
}

/// 子句短语：独立成行（Clause）或保持在行内（Inline），关键字按大小写选项输出
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Clause,
    Inline,
}

pub struct Phrase {
    pub words: &'static [&'static str],
    pub placement: Placement,
}

const fn clause(words: &'static [&'static str]) -> Phrase {
    Phrase {
        words,
        placement: Placement::Clause,
    }
}

const fn inline(words: &'static [&'static str]) -> Phrase {
    Phrase {
        words,
        placement: Placement::Inline,
    }
}

const UPSERT: &[Phrase] = &[
    clause(&["ON", "CONFLICT"]),
    clause(&["DO", "UPDATE", "SET"]),
    inline(&["DO", "NOTHING"]),
];

const MYSQL: &[Phrase] = &[clause(&["ON", "DUPLICATE", "KEY", "UPDATE"])];

const ORACLE: &[Phrase] = &[
    clause(&["CONNECT", "BY", "NOCYCLE"]),
    clause(&["CONNECT", "BY"]),
    clause(&["START", "WITH"]),
];

const BIGQUERY: &[Phrase] = &[clause(&["QUALIFY"])];

impl DialectKind {
    /// 交给格式化引擎的词法
    pub fn engine(self) -> Dialect {
        match self {
            Self::Postgresql => Dialect::PostgreSql,
            Self::Sqlserver => Dialect::SQLServer,
            // BigQuery 的数组下标与 ARRAY[...] 字面量和 PostgreSQL 一致
            Self::Bigquery => Dialect::PostgreSql,
            Self::Generic | Self::Mysql | Self::Sqlite | Self::Oracle => Dialect::Generic,
        }
    }

    /// # 开头的行注释
    pub fn hash_comments(self) -> bool {
        matches!(self, Self::Mysql | Self::Bigquery)
    }

    /// 字符串中的 \' \" 转义
    pub fn backslash_escapes(self) -> bool {
        matches!(self, Self::Mysql | Self::Bigquery)
    }

    /// [方括号] 是引号标识符（其余方言中是数组语法）
    pub fn bracket_identifiers(self) -> bool {
        matches!(self, Self::Generic | Self::Sqlserver | Self::Sqlite)
    }

    /// 引擎不认识方括号标识符时需要保护
    pub fn protect_brackets(self) -> bool {
        self == Self::Sqlite
    }

    /// '''三引号''' 与 r'' / b'' 前缀字符串
    pub fn bigquery_strings(self) -> bool {
        self == Self::Bigquery
    }

    /// q'[...]' 字符串
    pub fn q_quotes(self) -> bool {
        self == Self::Oracle
    }

    /// ARRAY<...> / STRUCT<...> 类型参数
    pub fn angle_types(self) -> bool {
        self == Self::Bigquery
    }

    /// $tag$ 字符串
    pub fn dollar_quotes(self) -> bool {
        !matches!(self, Self::Sqlserver | Self::Mysql | Self::Oracle)
    }

    pub fn phrases(self) -> &'static [Phrase] {
        match self {
            Self::Mysql => MYSQL,
            Self::Oracle => ORACLE,
            Self::Bigquery => BIGQUERY,
            Self::Postgresql | Self::Sqlite => UPSERT,
            Self::Generic | Self::Sqlserver => &[],
        }
    }

    /// 引擎关键字表之外、按大小写选项处理的方言关键字（均为大写）；
    /// 只收录几乎不会用作标识符的词，避免误改列名
    pub fn keywords(self) -> &'static [&'static str] {
        match self {
            Self::Mysql => &[
                "AUTO_INCREMENT",
                "UNSIGNED",
                "ZEROFILL",
                "STRAIGHT_JOIN",
                "SQL_CALC_FOUND_ROWS",
                "IGNORE",
                "REGEXP",
                "RLIKE",
                "XOR",
                "DIV",
                "DUPLICATE",
            ],
            Self::Sqlite => &[
                "AUTOINCREMENT",
                "PRAGMA",
                "VACUUM",
                "GLOB",
                "ATTACH",
                "DETACH",
                "REINDEX",
                "CONFLICT",
                "ABORT",
                "STRICT",
                "WITHOUT",
            ],
            Self::Oracle => &[
                "PRIOR",
                "NOCYCLE",
                "CONNECT_BY_ROOT",
                "SYSDATE",
                "SYSTIMESTAMP",
                "ROWNUM",
                "MINUS",
                "VARCHAR2",
                "NVARCHAR2",
                "PLS_INTEGER",
                "BULK",
                "COLLECT",
                "FORALL",
                "EXCEPTION",
                "ELSIF",
                "LOOP",
                "PRAGMA",
                "RETURNING",
            ],
            Self::Bigquery => &[
                "STRUCT",
                "ARRAY",
                "UNNEST",
                "SAFE_CAST",
                "INT64",
                "FLOAT64",
                "BIGNUMERIC",
                "BOOL",
                "BYTES",
                "GEOGRAPHY",
                "ASSERT",
                "LOOP",
                "ITERATE",
                "LEAVE",
                "PIVOT",
                "UNPIVOT",
                "TABLESAMPLE",
                "QUALIFY",
            ],
            Self::Postgresql => &["CONFLICT", "NOTHING", "RETURNING", "ILIKE"],
            Self::Generic | Self::Sqlserver => &[],
        }
    }
}
