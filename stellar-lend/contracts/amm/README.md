# StellarLend AMM Integration Contract

This contract provides Automated Market Maker (AMM) integration for the StellarLend protocol, enabling automated swaps and liquidity operations within lending operations.

## Features

- **Multi-Protocol Support**: Integrates with multiple AMM protocols
- **Automated Swaps**: Execute token swaps with slippage protection
- **Liquidity Operations**: Add/remove liquidity from AMM pools
- **Callback Validation**: Secure callback handling with replay protection
- **Event Emission**: Comprehensive event logging for all operations
- **Collateral Optimization**: Auto-swap functionality for optimal collateral ratios

## Key Functions

### Admin Functions
- `initialize_amm_settings`: Set up AMM parameters
- `add_amm_protocol`: Register new AMM protocols
- `update_amm_settings`: Modify AMM settings

### User Functions
- `execute_swap`: Perform token swaps
- `add_liquidity`: Add liquidity to pools
- `remove_liquidity`: Remove liquidity from pools
- `auto_swap_for_collateral`: Optimize collateral ratios

### Protocol Functions
- `validate_amm_callback`: Validate AMM protocol callbacks

## Security Features

- Slippage protection with configurable tolerances
- Callback validation with nonce-based replay protection and deadline (expiry) checks
- Admin-only configuration functions
- Comprehensive parameter validation
- Emergency pause functionality integration (`swap_enabled` / `liquidity_enabled` in settings)

## Trust Boundaries

- **Admin**: `initialize_amm_settings`, `add_amm_protocol`, and `update_amm_settings` configure the router. The stored `Admin` address must match the caller for updates (see contract implementation).
- **Registered AMM protocols**: Only addresses present in the protocol map may participate. Each entry includes an `enabled` flag; disabled protocols cannot complete callbacks.
- **`validate_amm_callback`**: Intended for **external** AMM contracts calling back into this router. The `caller` argument must **authorize** the invocation (Soroban `require_auth` on the protocol address) so arbitrary users cannot spoof a registered protocol. Validation checks: registered + enabled protocol, `ledger_timestamp <= deadline`, and a per-user monotonic nonce (replay attempts fail after the first successful consume).
- **Internal mock execution**: The bundled mock path validates callbacks via the same nonce and deadline rules but **without** requiring protocol auth, because the router invokes itself during tests/simulation. Integrating a real on-chain AMM should route through the external protocol contract and use `validate_amm_callback` from there; remove redundant internal validation to avoid consuming the nonce twice.
- **Tokens**: This crate’s mock swap/liquidity logic does not perform actual token transfers; production integrations must compose token contracts and allowance flows separately.

## Events

- `swap_executed`: Token swap details
- `liquidity_added`: Liquidity addition events
- `liquidity_removed`: Liquidity removal events
- `amm_operation`: General AMM operation tracking
- `callback_validated`: Callback validation events

## Usage

The AMM contract is designed to work seamlessly with the main StellarLend lending protocol, providing automated market making capabilities for optimal capital efficiency.