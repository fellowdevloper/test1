use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Deserialize)]
pub struct Transaction {
    pub r#type: TransactionType,
    pub client: u16,
    pub tx: u32,
    pub amount: Option<Decimal>,
}

#[derive(Debug, Default)]
pub struct Account {
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
}

impl Account {
    pub fn total(&self) -> Decimal {
        self.available + self.held
    }
}

pub struct Engine {
    pub accounts: HashMap<u16, Account>,
    tx_history: HashMap<u32, (u16, Decimal, TransactionType)>,
    disputed: HashMap<u32, bool>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            tx_history: HashMap::new(),
            disputed: HashMap::new(),
        }
    }

    pub fn process(&mut self, tx: Transaction) {
        let account = self.accounts.entry(tx.client).or_default();

        if account.locked {
            return;
        }

        match tx.r#type {
            TransactionType::Deposit => {
                let amount = tx.amount.unwrap_or_default();
                account.available += amount;
                self.tx_history.insert(tx.tx, (tx.client, amount, TransactionType::Deposit));
            }
            TransactionType::Withdrawal => {
                let amount = tx.amount.unwrap_or_default();
                if account.available >= amount {
                    account.available -= amount;
                    self.tx_history.insert(tx.tx, (tx.client, amount, TransactionType::Withdrawal));
                }
            }
            TransactionType::Dispute => {
                if self.disputed.contains_key(&tx.tx) {
                    return;
                }
                if let Some(&(client, amount, tx_type)) = self.tx_history.get(&tx.tx) {
                    if client == tx.client {
                        if tx_type == TransactionType::Deposit {
                            account.available -= amount;
                        }
                        account.held += amount;
                        self.disputed.insert(tx.tx, true);
                    }
                }
            }
            TransactionType::Resolve => {
                if self.disputed.get(&tx.tx) == Some(&true) {
                    if let Some(&(client, amount, tx_type)) = self.tx_history.get(&tx.tx) {
                        if client == tx.client {
                            account.held -= amount;
                            if tx_type == TransactionType::Deposit {
                                account.available += amount;
                            }
                            self.disputed.insert(tx.tx, false);
                        }
                    }
                }
            }
            TransactionType::Chargeback => {
                if self.disputed.get(&tx.tx) == Some(&true) {
                    if let Some(&(client, amount, _)) = self.tx_history.get(&tx.tx) {
                        if client == tx.client {
                            account.held -= amount;
                            account.locked = true;
                            self.disputed.insert(tx.tx, false);
                        }
                    }
                }
            }
        }
    }

    pub fn process_all(&mut self, transactions: impl Iterator<Item = Transaction>) {
        for tx in transactions {
            self.process(tx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn deposit(client: u16, tx: u32, amount: Decimal) -> Transaction {
        Transaction { r#type: TransactionType::Deposit, client, tx, amount: Some(amount) }
    }

    fn withdrawal(client: u16, tx: u32, amount: Decimal) -> Transaction {
        Transaction { r#type: TransactionType::Withdrawal, client, tx, amount: Some(amount) }
    }

    fn dispute(client: u16, tx: u32) -> Transaction {
        Transaction { r#type: TransactionType::Dispute, client, tx, amount: None }
    }

    fn resolve(client: u16, tx: u32) -> Transaction {
        Transaction { r#type: TransactionType::Resolve, client, tx, amount: None }
    }

    fn chargeback(client: u16, tx: u32) -> Transaction {
        Transaction { r#type: TransactionType::Chargeback, client, tx, amount: None }
    }

    #[test]
    fn test_basic_tx_history() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(1.0)));
        engine.process(deposit(1, 2, dec!(2.0)));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(3.0));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(3.0));
        assert!(!account.locked);
    }

    #[test]
    fn test_withdrawal_sufficient_funds() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(withdrawal(1, 2, dec!(3.0)));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(2.0));
        assert_eq!(account.total(), dec!(2.0));
    }

    #[test]
    fn test_withdrawal_insufficient_funds() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(2.0)));
        engine.process(withdrawal(1, 2, dec!(5.0)));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(2.0));
        assert_eq!(account.total(), dec!(2.0));
    }

    #[test]
    fn test_dispute() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(dispute(1, 1));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(5.0));
        assert_eq!(account.total(), dec!(5.0));
        assert!(!account.locked);
    }

    #[test]
    fn test_resolve() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(dispute(1, 1));
        engine.process(resolve(1, 1));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(5.0));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(5.0));
        assert!(!account.locked);
    }

    #[test]
    fn test_chargeback() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(dispute(1, 1));
        engine.process(chargeback(1, 1));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(0));
        assert!(account.locked);
    }

    #[test]
    fn test_locked_account_ignores_transactions() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(dispute(1, 1));
        engine.process(chargeback(1, 1));
        engine.process(deposit(1, 2, dec!(10.0)));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.total(), dec!(0));
        assert!(account.locked);
    }

    #[test]
    fn test_dispute_nonexistent_tx() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(dispute(1, 99));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(5.0));
        assert_eq!(account.held, dec!(0));
    }

    #[test]
    fn test_resolve_without_dispute() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(resolve(1, 1));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(5.0));
        assert_eq!(account.held, dec!(0));
    }

    #[test]
    fn test_chargeback_without_dispute() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(chargeback(1, 1));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(5.0));
        assert!(!account.locked);
    }

    #[test]
    fn test_chargeback_nonexistent_tx() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(chargeback(1, 99));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(5.0));
        assert!(!account.locked);
    }

    #[test]
    fn test_resolve_nonexistent_tx() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(resolve(1, 99));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(5.0));
        assert_eq!(account.held, dec!(0));
    }

    #[test]
    fn test_duplicate_dispute() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(10.0)));
        engine.process(dispute(1, 1));
        engine.process(dispute(1, 1));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(10.0));
        assert_eq!(account.total(), dec!(10.0));
    }

    #[test]
    fn test_duplicate_resolve() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(10.0)));
        engine.process(dispute(1, 1));
        engine.process(resolve(1, 1));
        engine.process(resolve(1, 1));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(10.0));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(10.0));
    }

    #[test]
    fn test_duplicate_chargeback() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(10.0)));
        engine.process(dispute(1, 1));
        engine.process(chargeback(1, 1));
        engine.process(chargeback(1, 1));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(0));
        assert!(account.locked);
    }

    #[test]
    fn test_withdrawal_exact_balance() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(withdrawal(1, 2, dec!(5.0)));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.total(), dec!(0));
    }

    #[test]
    fn test_deposit_zero_creates_account() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(0)));
        assert!(engine.accounts.contains_key(&1));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.total(), dec!(0));
    }

    #[test]
    fn test_dispute_withdrawal() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(10.0)));
        engine.process(withdrawal(1, 2, dec!(4.0)));
        engine.process(dispute(1, 2));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(6.0));
        assert_eq!(account.held, dec!(4.0));
        assert_eq!(account.total(), dec!(10.0));
    }

    #[test]
    fn test_resolve_disputed_withdrawal() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(10.0)));
        engine.process(withdrawal(1, 2, dec!(4.0)));
        engine.process(dispute(1, 2));
        engine.process(resolve(1, 2));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(6.0));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(6.0));
    }

    #[test]
    fn test_chargeback_disputed_withdrawal() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(10.0)));
        engine.process(withdrawal(1, 2, dec!(4.0)));
        engine.process(dispute(1, 2));
        engine.process(chargeback(1, 2));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(6.0));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(6.0));
        assert!(account.locked);
    }

    #[test]
    fn test_redispute_after_resolve() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(10.0)));
        engine.process(dispute(1, 1));
        engine.process(resolve(1, 1));
        engine.process(dispute(1, 1));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(10.0));
        assert_eq!(account.held, dec!(0));
        assert_eq!(account.total(), dec!(10.0));
    }

    #[test]
    fn test_dispute_wrong_client() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(dispute(2, 1));
        let a1 = &engine.accounts[&1];
        assert_eq!(a1.available, dec!(5.0));
        assert_eq!(a1.held, dec!(0));
    }

    #[test]
    fn test_dispute_causes_negative_available() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(10.0)));
        engine.process(withdrawal(1, 2, dec!(8.0)));
        engine.process(dispute(1, 1));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(-8.0));
        assert_eq!(account.held, dec!(10.0));
        assert_eq!(account.total(), dec!(2.0));
    }

    #[test]
    fn test_withdrawal_while_funds_held() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(10.0)));
        engine.process(dispute(1, 1));
        engine.process(withdrawal(1, 2, dec!(5.0)));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(10.0));
        assert_eq!(account.total(), dec!(10.0));
    }

    #[test]
    fn test_multiple_disputes_different_txs() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(deposit(1, 2, dec!(3.0)));
        engine.process(dispute(1, 1));
        engine.process(dispute(1, 2));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(8.0));
        assert_eq!(account.total(), dec!(8.0));
    }

    #[test]
    fn test_resolve_wrong_client() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(dispute(1, 1));
        engine.process(resolve(2, 1));
        let a1 = &engine.accounts[&1];
        assert_eq!(a1.available, dec!(0));
        assert_eq!(a1.held, dec!(5.0));
    }

    #[test]
    fn test_chargeback_wrong_client() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(5.0)));
        engine.process(dispute(1, 1));
        engine.process(chargeback(2, 1));
        let a1 = &engine.accounts[&1];
        assert_eq!(a1.available, dec!(0));
        assert_eq!(a1.held, dec!(5.0));
        assert!(!a1.locked);
    }

    #[test]
    fn test_dispute_failed_withdrawal() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(2.0)));
        engine.process(withdrawal(1, 2, dec!(5.0)));
        engine.process(dispute(1, 2));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(2.0));
        assert_eq!(account.held, dec!(0));
    }

    #[test]
    fn test_empty_engine() {
        let engine = Engine::new();
        assert!(engine.accounts.is_empty());
    }

    #[test]
    fn test_locked_account_blocks_withdrawal() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, dec!(10.0)));
        engine.process(deposit(1, 2, dec!(5.0)));
        engine.process(dispute(1, 1));
        engine.process(chargeback(1, 1));
        engine.process(withdrawal(1, 3, dec!(1.0)));
        let account = &engine.accounts[&1];
        assert_eq!(account.available, dec!(5.0));
        assert_eq!(account.total(), dec!(5.0));
        assert!(account.locked);
    }
}
