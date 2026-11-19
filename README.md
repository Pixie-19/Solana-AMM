
# Solana AMM

A Constant Product Automated Market Maker (AMM) built on Solana using the Anchor framework and Rust.

The program allows users to create liquidity pools, deposit and withdraw liquidity, and swap tokens. It also implements configurable swap fees collected by a treasury account.

## Features

- Initialize AMM liquidity pools.
- Deposit two tokens into a liquidity pool.
- Receive LP tokens representing liquidity ownership.
- Withdraw liquidity by burning LP tokens.
- Swap between Token X and Token Y.
- Calculate swap prices using the Constant Product Market Maker formula.
- Collect configurable swap fees through treasury token accounts.
- Validate amounts, liquidity, fees, and slippage.
- Checked arithmetic to reduce overflow and underflow risks.

## Tech Stack

- Solana
- Rust
- Anchor Framework
- SPL Token Program
- LiteSVM

## AMM Architecture

Each pool maintains:

- Token X mint
- Token Y mint
- LP token mint
- Token X vault
- Token Y vault
- Treasury
- Configured swap fee

The pool account is a Program Derived Address (PDA). The pool PDA controls the LP token mint and the pool vaults.

## Constant Product Market Maker

The AMM follows the constant product formula:

`x * y = k`

Where:

- `x` = Token X reserve
- `y` = Token Y reserve
- `k` = Constant product of the reserves

For a swap, the output amount is calculated using:

`amount_out = (reserve_out * net_input) / (reserve_in + net_input)`

The fee is deducted from the input amount before calculating the swap output.

## Swap Fees and Treasury

Swap fees are configured in basis points (BPS).

- 1 BPS = 0.01%
- 30 BPS = 0.30%
- Maximum permitted fee = 1,000 BPS (10%)

The calculated fee is transferred to the corresponding treasury token account, while the remaining input is transferred to the pool vault.

## Instructions

| Instruction | Description |
|---|---|
| `initialize` | Creates a pool and its associated accounts |
| `deposit` | Deposits Token X and Token Y and mints LP tokens |
| `withdraw` | Burns LP tokens and returns the proportional reserves |
| `swap` | Swaps one token for another and collects the fee |

## Project Structure

```text
solana_amm/
├── programs/
│   └── solana_amm/
│       ├── src/
│       │   ├── instructions/
│       │   │   ├── initialize.rs
│       │   │   ├── deposit.rs
│       │   │   ├── withdraw.rs
│       │   │   └── swap.rs
│       │   ├── math/
│       │   │   └── cpmm.rs
│       │   ├── constants.rs
│       │   ├── error.rs
│       │   ├── instructions.rs
│       │   ├── state.rs
│       │   └── lib.rs
│       └── tests/
│           └── test_initialize.rs
├── Anchor.toml
├── Cargo.toml
└── rust-toolchain.toml
```

## Setup

Install Rust, Solana CLI, and Anchor before building the project.

Clone the repository:

```bash
git clone https://github.com/Pixie-19/Solana-AMM.git
cd Solana-AMM/solana_amm
```

Build:

```bash
anchor build
```

Run the test suite:

```bash
cargo test -- --nocapture
```

## Tests

The project includes unit tests for CPMM calculations and integration tests using LiteSVM.

Current test coverage includes:

- CPMM swap calculation with fees.
- Zero-input validation.
- Zero-reserve validation.
- Pool initialization.
- Invalid fee rejection.
- Liquidity deposit.
- Liquidity withdrawal.
- Token swap and treasury fee collection.

## Limitations and Assumptions

- Uses the classic SPL Token Program.
- LP tokens use 6 decimals.
- Swap fee rounding uses integer arithmetic.
- Pool token ordering is determined by the initialization arguments.
- Minimum locked LP liquidity is not currently implemented.
- Additional adversarial and edge-case testing is recommended.

## Future Improvements

- Minimum locked LP liquidity.
- Additional security and economic attack testing.
- Support for tokens with different decimal configurations.
- More comprehensive negative integration tests.
- Additional pool management and monitoring functionality.

## Author

**Rishita Seal**

GitHub: [Pixie-19](https://github.com/Pixie-19)
