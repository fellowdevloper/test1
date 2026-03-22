use rust_decimal_macros::dec;
use test1::{Engine, Transaction};

fn run_csv(input: &str) -> Engine {
    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(input.as_bytes());

    let transactions = rdr
        .deserialize::<Transaction>()
        .filter_map(|r| r.ok());

    let mut engine = Engine::new();
    engine.process_all(transactions);
    engine
}

#[test]
fn test_multiple_clients() {
    let engine = run_csv(
        "type,client,tx,amount\n\
         deposit,1,1,1.0\n\
         deposit,2,2,2.0\n\
         deposit,1,3,2.0\n\
         withdrawal,1,4,1.5\n\
         withdrawal,2,5,3.0\n",
    );
    let a1 = &engine.accounts[&1];
    assert_eq!(a1.available, dec!(1.5));
    assert_eq!(a1.total(), dec!(1.5));

    let a2 = &engine.accounts[&2];
    assert_eq!(a2.available, dec!(2.0));
    assert_eq!(a2.total(), dec!(2.0));
}

#[test]
fn test_whitespace_handling() {
    let engine = run_csv(
        "type, client, tx, amount\n\
         deposit, 1, 1, 1.5000\n",
    );
    let account = &engine.accounts[&1];
    assert_eq!(account.available, dec!(1.5));
}

#[test]
fn test_full_dispute_lifecycle_csv() {
    let engine = run_csv(
        "type,client,tx,amount\n\
         deposit,1,1,10.0\n\
         dispute,1,1,\n\
         resolve,1,1,\n\
         deposit,1,2,5.0\n\
         dispute,1,2,\n\
         chargeback,1,2,\n",
    );
    let account = &engine.accounts[&1];
    assert_eq!(account.available, dec!(10.0));
    assert_eq!(account.held, dec!(0));
    assert_eq!(account.total(), dec!(10.0));
    assert!(account.locked);
}

#[test]
fn test_four_decimal_precision() {
    let engine = run_csv(
        "type,client,tx,amount\n\
         deposit,1,1,1.1234\n\
         deposit,1,2,2.5678\n\
         withdrawal,1,3,0.5000\n",
    );
    let account = &engine.accounts[&1];
    assert_eq!(account.available, dec!(3.1912));
    assert_eq!(account.total(), dec!(3.1912));
}
