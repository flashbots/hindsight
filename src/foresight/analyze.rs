use crate::{
    interfaces::{SimArbResultBatch, TokenPair},
    Result,
};
use ethers::utils::format_ether;
use statistical::{mean, standard_deviation};

/// Stats derived from a set of arbs.
#[derive(Clone, Debug)]
pub struct ArbStat {
    pub pair_tokens: TokenPair,
    /// Amounts are represented as floats (by 1/1e18).
    pub amount_in_mean: f64,
    pub amount_in_std_dev: f64,
}

/// EXPECTS ARBS TO BE FILTERED: `arbs` are all assumed to be for the same token pair.
pub fn analyze_arbs(arbs: &Vec<SimArbResultBatch>) -> Result<ArbStat> {
    // sort arbs by profit to get array of best arbs from each batch
    // then reduce to amount_in
    let amounts = arbs
        .iter()
        .map(|arb| {
            let mut sorted_res = arb.results.clone();
            sorted_res.sort_by(|a, b| a.backrun_trade.profit.cmp(&b.backrun_trade.profit));
            sorted_res
                .last()
                .expect("err: failed to sort arbs")
                .to_owned()
        })
        .map(|arb| {
            let n = format_ether(arb.backrun_trade.amount_in)
                .parse::<f64>()
                .unwrap();
            n
        })
        .collect::<Vec<f64>>();

    let xy_mean = mean(&amounts);
    let xy_sd = standard_deviation(&amounts, Some(xy_mean));

    Ok(ArbStat {
        pair_tokens: arbs[0].results[0].user_trade.tokens.to_owned(),
        amount_in_mean: xy_mean,
        amount_in_std_dev: xy_sd,
    })
}

// tests
#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_arbs() -> Vec<SimArbResultBatch> {
        vec![
            SimArbResultBatch::test_example(),
            SimArbResultBatch::test_example(),
            SimArbResultBatch::test_example(),
            SimArbResultBatch::test_example(),
            SimArbResultBatch::test_example(),
            SimArbResultBatch::test_example(),
        ]
    }

    #[test]
    fn test_analyze_arbs() -> Result<()> {
        let result = analyze_arbs(&get_test_arbs()).unwrap();
        assert!(result.amount_in_mean > 1.099999 && result.amount_in_mean < 1.1);
        assert!(result.amount_in_std_dev > 0.0 && result.amount_in_std_dev < 0.000001);
        Ok(())
    }
}
