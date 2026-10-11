//! The simulator, driven by the crate's own client over TCP.

use super::{Fault, Simulator, SimulatorConfig};
use crate::operations::{
    CashInOutRequest, PrintMode, PrintReceiptRequest, PrintReturnReceiptRequest, ReceiptItem,
};
use crate::{Client, Decimal, Error, InMemorySeq, ServerErrorKind};
use std::net::{TcpListener, TcpStream};

type TestClient = Client<TcpStream, InMemorySeq>;

fn start() -> (Simulator, std::net::SocketAddr) {
    let simulator = Simulator::new(SimulatorConfig::default());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let serving = simulator.clone();
    std::thread::spawn(move || serving.serve(&listener));
    (simulator, address)
}

fn connect(address: std::net::SocketAddr) -> TestClient {
    Client::new(
        TcpStream::connect(address).unwrap(),
        "simulator",
        InMemorySeq::default(),
    )
}

fn logged_in(address: std::net::SocketAddr) -> TestClient {
    let mut client = connect(address);
    client.login(1, "1234").unwrap();
    client
}

fn item(quantity: i64, price: i64) -> ReceiptItem {
    ReceiptItem {
        dep: 1,
        qty: Decimal::from(quantity),
        price: Decimal::from(price),
        product_code: "p1".to_owned(),
        product_name: "Water".to_owned(),
        adg_code: Some("2201".to_owned()),
        unit: "pcs".to_owned(),
        discount: None,
        discount_kind: None,
        additional_discount: None,
        additional_discount_kind: None,
    }
}

fn sale(items: Vec<ReceiptItem>, cash: i64, card: i64) -> PrintReceiptRequest {
    PrintReceiptRequest {
        mode: PrintMode::Products,
        paid_amount: Decimal::from(cash),
        paid_amount_card: Decimal::from(card),
        partial_amount: Decimal::ZERO,
        pre_payment_amount: Decimal::ZERO,
        dep: None,
        partner_tin: None,
        use_ext_pos: card > 0,
        payment_system: None,
        rrn: None,
        terminal_id: None,
        e_marks: Vec::new(),
        items,
    }
}

fn full_return(receipt: i64) -> PrintReturnReceiptRequest {
    PrintReturnReceiptRequest {
        crn: "00000001".to_owned(),
        return_ticket_id: u64::try_from(receipt).unwrap(),
        cash_amount_for_return: None,
        card_amount_for_return: None,
        pre_payment_amount_for_return: None,
        rrn: None,
        terminal_id: None,
        e_marks: Vec::new(),
        return_item_list: Vec::new(),
    }
}

fn refusal(outcome: Result<impl std::fmt::Debug, Error>) -> ServerErrorKind {
    match outcome {
        Err(Error::Server { kind, .. }) => kind,
        other => panic!("expected the device to refuse, got {other:?}"),
    }
}

#[test]
fn lists_its_operators_and_departments_to_whoever_has_the_password() {
    let (_simulator, address) = start();

    let directory = connect(address).list_operators_and_departments().unwrap();

    assert_eq!(directory.operators.len(), 1);
    assert_eq!(directory.departments.len(), 1);
}

#[test]
fn refuses_a_wrong_password_and_a_wrong_pin() {
    let (_simulator, address) = start();
    let stream = TcpStream::connect(address).unwrap();
    let mut stranger = Client::new(stream, "not the password", InMemorySeq::default());

    assert_eq!(
        refusal(stranger.list_operators_and_departments()),
        ServerErrorKind::CryptographicError
    );
    assert_eq!(
        refusal(connect(address).login(1, "0000")),
        ServerErrorKind::BadOperatorPassword
    );
    assert_eq!(
        refusal(connect(address).login(7, "1234")),
        ServerErrorKind::NoSuchOperator
    );
}

#[test]
fn registers_a_sale_and_returns_it_once() {
    let (simulator, address) = start();
    let mut client = logged_in(address);

    let receipt = client
        .print_receipt(sale(vec![item(3, 10)], 10, 20))
        .unwrap();
    assert_eq!(receipt.total, Decimal::from(30));
    assert_eq!(receipt.crn, "00000001");

    let looked_up = client
        .get_returnable_receipt(receipt.rseq.to_string(), "00000001")
        .unwrap();
    assert_eq!(looked_up.cash, Some(Decimal::from(10)));
    assert_eq!(looked_up.card, Some(Decimal::from(20)));
    assert_eq!(looked_up.totals.len(), 1);

    let returned = client
        .print_return_receipt(full_return(receipt.rseq))
        .unwrap();
    assert_eq!(returned.total, Decimal::from(30));
    assert_eq!(simulator.receipts_registered(), 2);

    assert_eq!(
        refusal(client.print_return_receipt(full_return(receipt.rseq))),
        ServerErrorKind::ReceiptAlreadyReturned
    );
    assert_eq!(
        refusal(client.print_return_receipt(full_return(returned.rseq))),
        ServerErrorKind::ReturnReceiptIsReturn
    );
}

#[test]
fn gives_change_for_cash_over_the_total() {
    let (_simulator, address) = start();

    let receipt = logged_in(address)
        .print_receipt(sale(vec![item(1, 700)], 1000, 0))
        .unwrap();

    assert_eq!(receipt.total, Decimal::from(700));
    assert_eq!(receipt.change, Decimal::from(300));
}

#[test]
fn refuses_a_receipt_a_register_would_refuse() {
    let (simulator, address) = start();
    let mut client = logged_in(address);

    let mut elsewhere = item(1, 10);
    elsewhere.dep = 9;
    let mut uncoded = item(1, 10);
    uncoded.adg_code = None;

    assert_eq!(
        refusal(client.print_receipt(sale(vec![elsewhere], 10, 0))),
        ServerErrorKind::NoSuchDepartment
    );
    assert_eq!(
        refusal(client.print_receipt(sale(vec![uncoded], 10, 0))),
        ServerErrorKind::AtgCodeRequired
    );
    assert_eq!(
        refusal(client.print_receipt(sale(vec![item(1, 10)], 5, 0))),
        ServerErrorKind::PaidLessThanTotal
    );
    assert_eq!(
        refusal(client.print_receipt(sale(vec![item(1, 10)], 0, 20))),
        ServerErrorKind::CardAmountExceedsTotal
    );
    assert_eq!(
        refusal(client.print_return_receipt(full_return(41))),
        ServerErrorKind::ReturnReceiptNotFound
    );
    assert_eq!(simulator.receipts_registered(), 0);
}

#[test]
fn refuses_cash_that_is_not_an_amount() {
    let (_simulator, address) = start();
    let mut client = logged_in(address);
    let movement = |amount: i64| CashInOutRequest {
        amount: Decimal::from(amount),
        is_cash_in: false,
        cashier_id: None,
        description: String::new(),
    };

    client.cash_in_out(movement(500)).unwrap();
    assert_eq!(
        refusal(client.cash_in_out(movement(0))),
        ServerErrorKind::NonPositiveCashAmount
    );
}

/// A sequence provider that never moves.
struct Stuck;

impl crate::SequenceProvider for Stuck {
    fn next(&mut self) -> std::io::Result<i64> {
        Ok(1)
    }
}

#[test]
fn wants_a_sequence_that_moves_forward_within_a_session() {
    let (simulator, address) = start();
    let mut client = Client::new(TcpStream::connect(address).unwrap(), "simulator", Stuck);
    client.login(1, "1234").unwrap();
    client
        .print_receipt(sale(vec![item(1, 10)], 10, 0))
        .unwrap();

    assert_eq!(
        refusal(client.print_receipt(sale(vec![item(1, 10)], 10, 0))),
        ServerErrorKind::BadSequenceNumber
    );
    assert_eq!(simulator.receipts_registered(), 1);
}

#[test]
fn ends_a_session_when_another_login_starts_one() {
    let (_simulator, address) = start();
    let mut first = logged_in(address);
    first.print_receipt(sale(vec![item(1, 10)], 10, 0)).unwrap();

    // The second session numbers its requests from the start again.
    let mut second = logged_in(address);
    second
        .print_receipt(sale(vec![item(1, 10)], 10, 0))
        .unwrap();

    assert_eq!(
        refusal(first.print_receipt(sale(vec![item(1, 10)], 10, 0))),
        ServerErrorKind::SessionEncryptionError
    );
}

#[test]
fn refuses_once_when_told_to() {
    let (simulator, address) = start();
    let mut client = logged_in(address);
    simulator.inject(Fault::Refuse(145));

    assert_eq!(
        refusal(client.print_receipt(sale(vec![item(1, 10)], 10, 0))),
        ServerErrorKind::PrinterOutOfPaper
    );
    assert_eq!(simulator.receipts_registered(), 0);

    client
        .print_receipt(sale(vec![item(1, 10)], 10, 0))
        .unwrap();
    assert_eq!(simulator.receipts_registered(), 1);
}

#[test]
fn registers_and_loses_the_answer_when_told_to() {
    let (simulator, address) = start();
    let mut client = logged_in(address);
    simulator.inject(Fault::LoseAnswer);

    let outcome = client.print_receipt(sale(vec![item(1, 10)], 10, 0));

    assert!(matches!(outcome, Err(Error::Transport(_))), "{outcome:?}");
    assert_eq!(simulator.receipts_registered(), 1);
}
