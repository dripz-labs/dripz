# Security Notes

## Onchain program (private monorepo, referenced here)

- **Anchor 0.31** with PDA-based pool accounts.
- All state mutations pass through a `pool_authority` PDA. The authority's seeds are derived from `(pool_id, bump)` so anyone can verify a given account belongs to the expected pool.
- **Reentrancy**. Every instruction sets a `locked` flag at entry and unsets it on the happy path. Cross-program invocations are forbidden while `locked` is true.
- **Slippage**. Every buy / sell takes a `min_amount_out` argument that the caller signs. The on-chain math is bit-for-bit identical to `dripz-engine`, so SDK quotes and on-chain quotes agree.
- **Anti-snipe**. The `dripz-snipeguard` per-tx cap runs as the first check inside `buy` during the protected window. The commit-reveal flow uses SHA-256 over `(wallet || amount || nonce)`.
- **Vesting**. The vesting rate is computed at `start_ts` and frozen; there is no admin update path.

## Off-chain service

- Helius RPC keys live only in `HELIUS_RPC_URL` (server env). The web client uses the public `https://api.mainnet-beta.solana.com` RPC.
- The Jito block engine URL (`JITO_BLOCK_ENGINE_URL`) is server-only as well; bundle construction never leaves the backend.
- The Telegram bot token (`TELEGRAM_BOT_TOKEN`) is held by the bot service. The token has no `send_messages_in_groups` scope; only the bot can post to its private channel.
- CORS is allowlisted to four explicit origins (`https://dripz-web.vercel.app`, `https://dripz.fi`, `https://www.dripz.fi`, `http://localhost:3000`). Wildcards are forbidden and `allow_credentials=True`.

## Frontend

- The Solana wallet adapter only sees the public RPC endpoint. The DAS API and any Helius-enhanced calls are proxied through Next.js Route Handlers at `/api/das/*`.
- The build pipeline runs `grep -rE "helius-rpc\.com/\?api-key=|jito.*api-key=" .next/` before shipping; the build fails if a key leaked into the client bundle.

## Audit-style invariants

- **Weighted-pool invariant**. `B_i^{W_i} \cdot B_o^{W_o} = K` is checked at every swap by the on-chain program.
- **Monotonic curves**. `dripz-curves` has unit tests asserting weight is non-increasing across the entire window for Linear, Exponential, and Step curves; a fuzz test in the program test suite checks the same property on randomly sampled parameters.
- **Vesting**. Cliff + linear release; admin cannot rewrite the schedule.

## Threat model

| Threat | Mitigation |
| ------ | ---------- |
| Sniper bot races the open block | `dripz-snipeguard` per-tx cap + commit-reveal |
| MEV searcher sandwiches a buy | Jito bundle with DontFront + auto tip |
| Issuer rugs vesting | Vesting rate immutable after `start_ts` |
| Step curve transition front-run | Anchor sweep transaction inside the same Jito bundle |
| Fair discovery oracle gamed | 5-minute rolling window on buy pressure |
| Secret leak in client | Build-time grep + Next.js Route Handler proxy pattern |
