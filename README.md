# Toy Transactions Engine

A simple transactions engine that processes payments from a CSV file, crediting and debiting client accounts, and outputs final account balances.

## Usage

```bash
cargo run -- transactions.csv > accounts.csv
```

## Supported Transaction Types

- **Deposit** — credits funds to a client's available balance
- **Withdrawal** — debits funds from a client's available balance (fails silently if insufficient funds)
- **Dispute** — holds funds from a previously deposited transaction, moving them from available to held
- **Resolve** — releases disputed funds back to available
- **Chargeback** — withdraws disputed funds and freezes the account

## Correctness

### Type System

The engine uses Rust's type system to enforce correctness at compile time:

- `TransactionType` is an enum deserialized directly from CSV via serde, so only valid transaction types are accepted — invalid types result in a parse error rather than silent misbehavior.
- Client IDs are `u16` and transaction IDs are `u32`, matching the spec and preventing overflow at the type level.
- The `amount` field is `Option<Decimal>` (using `rust_decimal`), reflecting that disputes, resolves, and chargebacks do not carry an amount. `Decimal` ensures exact arithmetic with up to four decimal places, avoiding floating point rounding errors.

### Testing

The project includes both unit and integration tests.

**Unit tests** (`src/lib.rs`) exercise the `Engine::process()` method directly by constructing `Transaction` values in code. This tests the core logic in isolation without CSV parsing. Cases covered:

- Basic deposits accumulate correctly
- Withdrawals with sufficient and insufficient funds
- Dispute moves funds from available to held (deposits) or adds back to held (withdrawals)
- Resolve releases held funds back to available
- Chargeback removes held funds and locks the account
- Locked accounts reject all further transaction types (deposits, withdrawals, disputes, resolves, chargebacks)
- Disputes referencing nonexistent transactions are ignored
- Resolves and chargebacks without a prior dispute are ignored
- Disputes from a different client than the original transaction are ignored
- Duplicate disputes, resolves, and chargebacks on the same transaction are ignored
- Re-disputing a previously resolved or chargebacked transaction is not allowed
- Disputing a failed withdrawal (insufficient funds) is ignored
- Negative available balances are allowed when a dispute holds more than the current available

**Integration tests** (`tests/integration_test.rs`) exercise the full CSV-to-engine pipeline end-to-end, parsing CSV input (including whitespace variations) and verifying final account state. Cases covered:

- Multiple clients with interleaved deposits and withdrawals
- Whitespace and decimal precision handling in CSV input
- Full dispute lifecycle (deposit → dispute → resolve → deposit → dispute → chargeback)

## Assumptions

The spec leaves certain edge cases open to interpretation. Below are the assumptions we made and the reasoning behind each.

**Negative available balances are permitted.** If a client deposits 10, withdraws 8 (available=2), and then the original deposit of 10 is disputed, available goes to -8 while held becomes 10. This mirrors real-world banking where an account can go into overdraft when a dispute holds funds that have already been partially spent. The alternative — rejecting the dispute — would mean valid fraud claims could be blocked simply because the client spent some of the funds.

**A transaction can only be disputed once.** Once a transaction has been disputed and subsequently resolved or chargebacked, it cannot be disputed again. We treat the dispute lifecycle as a one-way process: undisputed → disputed → resolved/chargebacked (terminal). Allowing re-disputes could be exploited to repeatedly hold and release funds, and there is no clear spec guidance that re-disputes should be supported.

**Duplicate resolves and chargebacks are ignored.** If a resolve or chargeback is submitted for a transaction that is no longer under dispute (either already resolved or already chargebacked), it is silently ignored. The first resolution or chargeback is the authoritative outcome.

**Disputes, resolves, and chargebacks from the wrong client are ignored.** If client 2 attempts to dispute, resolve, or chargeback a transaction that belongs to client 1, the operation is silently ignored. A client should only be able to act on their own transactions.

**Failed withdrawals cannot be disputed.** If a withdrawal fails due to insufficient funds, it is not recorded in the transaction history. A subsequent dispute referencing that transaction ID is silently ignored, since the withdrawal never actually occurred.

**Negative available balances block withdrawals but not disputes.** A withdrawal requires `available >= amount` to succeed. However, a dispute can push available below zero. This means a client with negative available cannot withdraw, but further disputes on other transactions are still processed. This prevents clients from spending funds that are under review while allowing legitimate dispute processing to continue.

**Locked accounts reject all transaction types.** When a chargeback occurs, the account is frozen. All subsequent transactions — deposits, withdrawals, disputes, resolves, and chargebacks — are silently ignored. This is the most conservative interpretation: once an account is locked, it requires manual intervention outside the scope of this engine.

### Sample Data

The included `transactions.csv` file contains the sample data from the spec:

```
type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0
```

Expected output:

```
client,available,held,total,locked
1,1.5000,0.0000,1.5000,false
2,2.0000,0.0000,2.0000,false
```

Run all tests with:

```bash
cargo test
```
