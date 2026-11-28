//! What is left of the account the key belongs to, so that a question that
//! would spend it is asked with the cost in sight.

use crate::config::Ai;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::io::Read;

/// One currency's worth, as the account holds it. The amounts are strings,
/// as the server writes them, and are shown as written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    /// `CNY` or `USD`.
    pub currency: String,
    /// What can be spent: the grant not yet expired, and what was paid in.
    pub total_balance: String,
    /// The grant not yet expired.
    pub granted_balance: String,
    /// What was paid in.
    pub topped_up_balance: String,
}

impl Money {
    /// The amount written as the account's own currency writes it.
    pub fn amount(&self) -> String {
        match self.currency.as_str() {
            "CNY" => format!("¥{}", self.total_balance),
            "USD" => format!("${}", self.total_balance),
            other => format!("{} {other}", self.total_balance),
        }
    }
}

/// The account's balance, in each currency it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Balance {
    /// There is something to spend; a call is answered even at nothing.
    pub is_available: bool,
    #[serde(default)]
    pub balance_infos: Vec<Money>,
}

impl Balance {
    /// What the page shows first: the account's first currency.
    pub fn first(&self) -> Option<&Money> {
        self.balance_infos.first()
    }

    /// Whether what is left can be spent. A call is answered even at
    /// nothing, and is billed all the same.
    pub fn can_spend(&self) -> bool {
        self.is_available
    }
}

/// Where the balance is asked: the server named, and the one path every
/// server of this shape answers.
pub fn url(ai: &Ai) -> String {
    format!("{}/user/balance", ai.base_url.trim_end_matches('/'))
}

/// What a refused request says, read whole.
#[derive(Debug, Deserialize)]
struct Failure {
    error: Said,
}

#[derive(Debug, Deserialize)]
struct Said {
    #[serde(default)]
    message: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

/// Ask the server what the key's account holds.
pub fn ask(agent: &ureq::Agent, ai: &Ai) -> Result<Balance> {
    ensure!(!ai.api_key.trim().is_empty(), "no key is typed, so there is no account to ask about");
    let mut response = agent
        .get(&url(ai))
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {}", ai.api_key.trim()))
        .call()
        .context("ask the server about the balance")?;
    let status = response.status();
    let mut body = String::new();
    response.body_mut().with_config().limit(1 << 20).reader().read_to_string(&mut body).context("read the answer about the balance")?;
    if !status.is_success() {
        let said = serde_json::from_str::<Failure>(&body)
            .ok()
            .and_then(|failure| match (failure.error.message, failure.error.kind) {
                (Some(message), Some(kind)) => Some(format!("{message} ({kind})")),
                (Some(message), None) => Some(message),
                (None, kind) => kind,
            });
        match said {
            Some(said) => bail!("the server answered {}: {said}", status.as_u16()),
            None if body.trim().is_empty() => bail!("the server answered {}", status.as_u16()),
            None => bail!("the server answered {}: {}", status.as_u16(), body.trim()),
        }
    }
    serde_json::from_str(&body).with_context(|| format!("read the balance {body}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_answer_reads_as_the_account_holds_it() {
        let body = r#"{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"110.00","granted_balance":"10.00","topped_up_balance":"100.00"}]}"#;
        let balance: Balance = serde_json::from_str(body).unwrap();
        assert!(balance.is_available);
        assert!(balance.can_spend());
        assert_eq!(balance.balance_infos[0].total_balance, "110.00");
        assert_eq!(balance.first().map(Money::amount).as_deref(), Some("¥110.00"));
    }

    #[test]
    fn an_account_with_nothing_left_shows_what_it_holds() {
        let balance: Balance = serde_json::from_str(r#"{"is_available":false,"balance_infos":[{"currency":"USD","total_balance":"0.00","granted_balance":"0.00","topped_up_balance":"0.00"}]}"#).unwrap();
        assert!(!balance.can_spend());
        assert_eq!(balance.first().map(Money::amount).as_deref(), Some("$0.00"));
        let empty: Balance = serde_json::from_str(r#"{"is_available":false,"balance_infos":[]}"#).unwrap();
        assert!(empty.first().is_none(), "the page says so in its own words");
        let odd: Balance = serde_json::from_str(r#"{"is_available":true,"balance_infos":[{"currency":"JPY","total_balance":"900","granted_balance":"0","topped_up_balance":"900"}]}"#).unwrap();
        assert_eq!(odd.first().map(Money::amount).as_deref(), Some("900 JPY"));
    }

    #[test]
    fn the_balance_is_asked_of_the_server_named() {
        assert_eq!(url(&Ai::default()), "https://api.deepseek.com/user/balance");
        assert_eq!(url(&Ai { base_url: "https://api.deepseek.com/".into(), ..Ai::default() }), "https://api.deepseek.com/user/balance");
    }

    #[test]
    fn a_balance_without_a_key_is_not_asked() {
        let agent = ureq::Agent::config_builder().build().new_agent();
        assert!(ask(&agent, &Ai::default()).is_err());
    }
}
