//! `dripz` -- reference CLI for the Dripz LBP framework.
//!
//! The CLI front-end matches the npm `dripz-cli` package shipped from the
//! private monorepo. Three subcommands are available:
//!
//! - `design`    -- emit a sampled curve as JSON for the web designer
//! - `simulate`  -- run a pool through a curve with a synthetic demand series
//! - `backtest`  -- compare snipe-resistance metrics across two curves
//!
//! Everything below is pure integer math. The output JSON is consumed by the
//! Next.js Curve Designer, the Telegram bot, and the verifier tests.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use dripz_curves::{AnyCurve, CurveKind, CurveParams};
use dripz_engine::{PoolConfig, PoolState};
use dripz_snipeguard::{enforce_buy, MaxBuyConfig, RollingWindow};
use serde::Serialize;

mod backtest;
mod simulator;

use backtest::BacktestSummary;
use simulator::SimulationSample;

#[derive(Debug, Parser)]
#[command(name = "dripz", version, about = "Dripz LBP reference CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the sampled weight curve as JSON.
    Design(DesignArgs),
    /// Run a pool through the curve with a synthetic demand profile.
    Simulate(SimulateArgs),
    /// Compare two curves under an identical demand profile.
    Backtest(BacktestArgs),
    /// Show the per-tx and rolling-window guard decision for a single buy.
    Guard(GuardArgs),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CurveCli {
    Linear,
    Exponential,
    Step,
    Dutch,
    Fair,
}

impl From<CurveCli> for CurveKind {
    fn from(value: CurveCli) -> Self {
        match value {
            CurveCli::Linear => CurveKind::Linear,
            CurveCli::Exponential => CurveKind::Exponential,
            CurveCli::Step => CurveKind::Step,
            CurveCli::Dutch => CurveKind::Dutch,
            CurveCli::Fair => CurveKind::Fair,
        }
    }
}

#[derive(Debug, Parser)]
struct DesignArgs {
    #[arg(long, value_enum)]
    curve: CurveCli,
    #[arg(long, default_value_t = 990_000)]
    start_weight_micro: u64,
    #[arg(long, default_value_t = 500_000)]
    end_weight_micro: u64,
    #[arg(long, default_value_t = 604_800)]
    duration_secs: u64,
    #[arg(long)]
    exponential_k_micro: Option<u64>,
    #[arg(long, default_value_t = 41)]
    samples: usize,
}

#[derive(Debug, Parser)]
struct SimulateArgs {
    #[arg(long, value_enum, default_value = "linear")]
    curve: CurveCli,
    #[arg(long, default_value_t = 990_000)]
    start_weight_micro: u64,
    #[arg(long, default_value_t = 500_000)]
    end_weight_micro: u64,
    #[arg(long, default_value_t = 604_800)]
    duration_secs: u64,
    #[arg(long, default_value_t = 10_000_000_000)]
    initial_token_lamports: u64,
    #[arg(long, default_value_t = 1_000_000_000)]
    initial_quote_lamports: u64,
    #[arg(long, default_value_t = 30)]
    swap_fee_bps: u16,
    #[arg(long, default_value_t = 50)]
    points: usize,
}

#[derive(Debug, Parser)]
struct BacktestArgs {
    #[arg(long, value_enum, default_value = "linear")]
    baseline: CurveCli,
    #[arg(long, value_enum, default_value = "exponential")]
    candidate: CurveCli,
    #[arg(long, default_value_t = 990_000)]
    start_weight_micro: u64,
    #[arg(long, default_value_t = 500_000)]
    end_weight_micro: u64,
    #[arg(long, default_value_t = 604_800)]
    duration_secs: u64,
}

#[derive(Debug, Parser)]
struct GuardArgs {
    #[arg(long, default_value_t = 0)]
    launch_slot: u64,
    #[arg(long, default_value_t = 300)]
    protected_slots: u64,
    #[arg(long, default_value_t = 50)]
    max_share_bps: u16,
    #[arg(long, default_value_t = 10_000_000_000)]
    vault_balance: u64,
    #[arg(long, default_value_t = 5_000_000)]
    requested_amount: u64,
    #[arg(long, default_value_t = 100)]
    current_slot: u64,
}

#[derive(Debug, Serialize)]
struct DesignOutput {
    curve: String,
    samples: Vec<(u64, u64)>,
    start_weight_micro: u64,
    end_weight_micro: u64,
    duration_secs: u64,
}

#[derive(Debug, Serialize)]
struct SimulateOutput {
    curve: String,
    samples: Vec<SimulationSample>,
}

#[derive(Debug, Serialize)]
struct BacktestOutput {
    baseline: BacktestSummary,
    candidate: BacktestSummary,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Design(args) => design(args),
        Command::Simulate(args) => simulate(args),
        Command::Backtest(args) => backtest_cmd(args),
        Command::Guard(args) => guard(args),
    }
}

fn build_params(
    kind: CurveCli,
    start: u64,
    end: u64,
    duration: u64,
    exponential_k_micro: Option<u64>,
) -> CurveParams {
    let mut params = CurveParams::linear(start, end, duration);
    params.kind = CurveKind::from(kind);
    if let Some(k) = exponential_k_micro {
        params.exponential_k_micro = Some(k);
    } else if params.kind == CurveKind::Exponential {
        params.exponential_k_micro = Some(3_000_000);
    }
    if params.kind == CurveKind::Dutch {
        params.dutch_price_max_micro = Some(1_000_000);
        params.dutch_price_min_micro = Some(100_000);
    }
    if params.kind == CurveKind::Fair {
        params.fair_alpha_micro = Some(300_000);
    }
    params
}

fn design(args: DesignArgs) -> Result<()> {
    let params = build_params(
        args.curve,
        args.start_weight_micro,
        args.end_weight_micro,
        args.duration_secs,
        args.exponential_k_micro,
    );
    let curve = AnyCurve::from_params(&params).context("constructing curve")?;
    let samples = (0..args.samples)
        .map(|i| {
            let t = (i as u64 * args.duration_secs) / args.samples.saturating_sub(1).max(1) as u64;
            let w = curve.weight_token_micro(t).unwrap_or(args.end_weight_micro);
            (t, w)
        })
        .collect::<Vec<_>>();
    let output = DesignOutput {
        curve: format!("{:?}", curve.kind()),
        samples,
        start_weight_micro: args.start_weight_micro,
        end_weight_micro: args.end_weight_micro,
        duration_secs: args.duration_secs,
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn simulate(args: SimulateArgs) -> Result<()> {
    let params = build_params(
        args.curve,
        args.start_weight_micro,
        args.end_weight_micro,
        args.duration_secs,
        None,
    );
    let curve = AnyCurve::from_params(&params).context("constructing curve")?;
    let config = PoolConfig::new("DRIPZ", "USDC", args.swap_fee_bps, curve)
        .context("constructing pool config")?;
    let mut state = PoolState {
        balance_token_lamports: args.initial_token_lamports,
        balance_quote_lamports: args.initial_quote_lamports,
        weight_token_micro: args.start_weight_micro,
        weight_quote_micro: 1_000_000 - args.start_weight_micro,
        elapsed_secs: 0,
    };
    state.refresh_weights(&config)?;
    let samples = simulator::run(&config, &mut state, args.duration_secs, args.points)?;
    let output = SimulateOutput {
        curve: format!("{:?}", config.curve_kind()),
        samples,
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn backtest_cmd(args: BacktestArgs) -> Result<()> {
    let baseline_params = build_params(
        args.baseline,
        args.start_weight_micro,
        args.end_weight_micro,
        args.duration_secs,
        None,
    );
    let candidate_params = build_params(
        args.candidate,
        args.start_weight_micro,
        args.end_weight_micro,
        args.duration_secs,
        None,
    );
    let baseline =
        backtest::run(&baseline_params).map_err(|e| anyhow!("baseline backtest failed: {e}"))?;
    let candidate =
        backtest::run(&candidate_params).map_err(|e| anyhow!("candidate backtest failed: {e}"))?;
    let output = BacktestOutput {
        baseline,
        candidate,
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn guard(args: GuardArgs) -> Result<()> {
    let config = MaxBuyConfig {
        launch_slot: args.launch_slot,
        protected_slots: args.protected_slots,
        max_share_bps: args.max_share_bps,
    };
    let rolling = RollingWindow::new(86_400, args.vault_balance);
    let wallet = [0u8; 32];
    let decision = enforce_buy(
        &config,
        &rolling,
        args.vault_balance,
        args.requested_amount,
        &wallet,
        args.current_slot,
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&decision_to_json(decision))?
    );
    Ok(())
}

#[derive(Debug, Serialize)]
struct GuardDecisionJson {
    accepted: bool,
    reject_reason: Option<&'static str>,
    effective_cap_tokens: String,
}

fn decision_to_json(decision: dripz_snipeguard::EnforceBuyDecision) -> GuardDecisionJson {
    GuardDecisionJson {
        accepted: decision.accepted,
        reject_reason: decision.reject_reason,
        effective_cap_tokens: decision.effective_cap_tokens.to_string(),
    }
}
