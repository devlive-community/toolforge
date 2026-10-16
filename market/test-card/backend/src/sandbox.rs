//! 支付平台官方沙箱测试卡。只收录官方文档中明确列出的卡号，场景以键名返回由前端翻译。
//! 这些卡号只在对应平台的测试模式（测试 API 密钥）下有意义，不能修改后再声称兼容该平台。

use serde::Serialize;

use crate::brand::Brand;

pub const STRIPE_SOURCE: &str = "https://docs.stripe.com/testing?testing-method=card-numbers";
/// 与官方文档核对的日期
pub const VERIFIED: &str = "2026-10-16";

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    Success,
    Decline,
    Authentication,
    Fraud,
    Dispute,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Case {
    pub platform: &'static str,
    pub number: &'static str,
    pub brand: Brand,
    pub category: Category,
    /// 场景键名
    pub scenario: &'static str,
    /// 平台返回的错误码（如有）
    pub code: Option<&'static str>,
    pub source: &'static str,
    pub verified: &'static str,
}

const fn stripe(
    number: &'static str,
    brand: Brand,
    category: Category,
    scenario: &'static str,
    code: Option<&'static str>,
) -> Case {
    Case {
        platform: "stripe",
        number,
        brand,
        category,
        scenario,
        code,
        source: STRIPE_SOURCE,
        verified: VERIFIED,
    }
}

use Brand::*;
use Category::*;

pub const CASES: &[Case] = &[
    stripe("4242424242424242", Visa, Success, "visa", None),
    stripe("4000056655665556", Visa, Success, "visaDebit", None),
    stripe("5555555555554444", Mastercard, Success, "mastercard", None),
    stripe(
        "2223003122003222",
        Mastercard,
        Success,
        "mastercard2Series",
        None,
    ),
    stripe(
        "5200828282828210",
        Mastercard,
        Success,
        "mastercardDebit",
        None,
    ),
    stripe(
        "5105105105105100",
        Mastercard,
        Success,
        "mastercardPrepaid",
        None,
    ),
    stripe("378282246310005", Amex, Success, "amex", None),
    stripe("371449635398431", Amex, Success, "amex", None),
    stripe("6011111111111117", Discover, Success, "discover", None),
    stripe("6011000990139424", Discover, Success, "discover", None),
    stripe("6011981111111113", Discover, Success, "discoverDebit", None),
    stripe("3056930009020004", Diners, Success, "diners", None),
    stripe("36227206271667", Diners, Success, "diners14", None),
    stripe("3566002020360505", Jcb, Success, "jcb", None),
    stripe("6200000000000005", Unionpay, Success, "unionpay", None),
    stripe("6200000000000047", Unionpay, Success, "unionpayDebit", None),
    stripe("6205500000000000004", Unionpay, Success, "unionpay19", None),
    stripe(
        "4000000000000002",
        Visa,
        Decline,
        "genericDecline",
        Some("card_declined / generic_decline"),
    ),
    stripe(
        "4000000000009995",
        Visa,
        Decline,
        "insufficientFunds",
        Some("card_declined / insufficient_funds"),
    ),
    stripe(
        "4000000000009987",
        Visa,
        Decline,
        "lostCard",
        Some("card_declined / lost_card"),
    ),
    stripe(
        "4000000000009979",
        Visa,
        Decline,
        "stolenCard",
        Some("card_declined / stolen_card"),
    ),
    stripe(
        "4000000000000069",
        Visa,
        Decline,
        "expiredCard",
        Some("expired_card"),
    ),
    stripe(
        "4000000000000127",
        Visa,
        Decline,
        "incorrectCvc",
        Some("incorrect_cvc"),
    ),
    stripe(
        "4000000000000119",
        Visa,
        Decline,
        "processingError",
        Some("processing_error"),
    ),
    stripe(
        "4242424242424241",
        Visa,
        Decline,
        "incorrectNumber",
        Some("incorrect_number"),
    ),
    stripe(
        "4000000000006975",
        Visa,
        Decline,
        "velocityExceeded",
        Some("card_declined / card_velocity_exceeded"),
    ),
    stripe(
        "4000000000000341",
        Visa,
        Decline,
        "declineAfterAttach",
        None,
    ),
    stripe(
        "4000002500003155",
        Visa,
        Authentication,
        "authenticateUnlessSetUp",
        None,
    ),
    stripe(
        "4000002760003184",
        Visa,
        Authentication,
        "alwaysAuthenticate",
        None,
    ),
    stripe(
        "4000003800000446",
        Visa,
        Authentication,
        "alreadySetUp",
        None,
    ),
    stripe(
        "4000008260003178",
        Visa,
        Authentication,
        "insufficientAfterAuth",
        None,
    ),
    stripe(
        "4000000000003220",
        Visa,
        Authentication,
        "required3ds2",
        None,
    ),
    stripe(
        "4000008400001629",
        Visa,
        Authentication,
        "requiredDeclined",
        None,
    ),
    stripe(
        "4000000032200000",
        Visa,
        Authentication,
        "frictionless",
        None,
    ),
    stripe("4100000000000019", Visa, Fraud, "alwaysBlocked", None),
    stripe("4000000000004954", Visa, Fraud, "highestRisk", None),
    stripe("4000000000009235", Visa, Fraud, "elevatedRisk", None),
    stripe("4000000000000101", Visa, Fraud, "cvcCheckFails", None),
    stripe("4000000000000036", Visa, Fraud, "postalCheckFails", None),
    stripe("4000000000000259", Visa, Dispute, "fraudulent", None),
    stripe("4000000000002685", Visa, Dispute, "notReceived", None),
    stripe("4000000000001976", Visa, Dispute, "inquiry", None),
    stripe("4000000000005423", Visa, Dispute, "earlyFraudWarning", None),
];

#[cfg(test)]
#[path = "sandbox_test.rs"]
mod tests;
