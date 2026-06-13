//! Command dispatch and one handler per subcommand. Shared connection, session, and output
//! plumbing lives in [`crate::conn`].

use std::time::Instant;

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hdm_am::{
    CashInOutRequest, Error as HdmError, FiscalReportKind, FiscalReportRequest, PrintMode,
    PrintReceiptRequest, PrintReturnReceiptRequest, ReceiptItem, ReportFilter, ReturnItem,
    ServerErrorKind, SetupHeaderFooterRequest, TextLine, identify,
};
use rust_decimal::Decimal;

use crate::conn::{client, confirm, connect, emit, require, with_session};
use crate::format;
use crate::{
    CashArgs, CashDirection, Cli, Command, HeaderFooterArgs, LogoArgs, LookupReceiptArgs,
    ReceiptArgs, ReceiptMode, ReportArgs, ReportKind, ReturnArgs,
};

/// Parsed shape of the `header-footer --file` JSON.
#[derive(serde::Deserialize)]
struct HeaderFooterFile {
    #[serde(default)]
    headers: Vec<TextLine>,
    #[serde(default)]
    footers: Vec<TextLine>,
}

/// Route a parsed CLI invocation to its handler.
pub fn dispatch(cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Probe => probe(cli),
        Command::Operators => operators(cli),
        Command::Login => login(cli),
        Command::Datetime => datetime(cli),
        Command::PaymentSystems => payment_systems(cli),
        Command::Sample => sample(cli),
        Command::Receipt(args) => receipt(cli, args),
        Command::PrintLast => print_last(cli),
        Command::Report(args) => report(cli, args),
        Command::Cash(args) => cash(cli, args),
        Command::TimeSync => time_sync(cli),
        Command::Emark { code } => emark(cli, code),
        Command::LookupReceipt(args) => lookup_receipt(cli, args),
        Command::Return(args) => return_receipt(cli, args),
        Command::HeaderFooter(args) => header_footer(cli, args),
        Command::Logo(args) => logo(cli, args),
        Command::Bridge(args) => crate::bridge::dispatch(cli, args),
    }
}

// ---------------- Handlers ----------------

fn probe(cli: &Cli) -> Result<()> {
    let host = require(cli.host.as_deref(), "host", "HDM_HOST")?;
    let started = Instant::now();
    let mut stream = connect(cli)?;
    let connect_ms = started.elapsed().as_millis();

    // An unauthenticated identify proves the endpoint actually speaks HDM (not just that a port is
    // open). NotHdm and a mute endpoint are legitimate findings, not failures of the probe itself,
    // so they are reported via the result rather than bubbled up as errors.
    match identify(&mut stream) {
        Ok(id) => {
            let protocol = format!("{}.{}", id.protocol_version.0, id.protocol_version.1);
            let software = format!(
                "{}.{}.{}",
                id.software_version.0, id.software_version.1, id.software_version.2
            );
            let note = probe_code_note(id.response_code);
            let status = serde_json::json!({
                "host": host,
                "port": cli.port,
                "reachable": true,
                "responded": true,
                "is_hdm": true,
                "connect_ms": connect_ms,
                "protocol_version": protocol,
                "software_version": software,
                "response_code": id.response_code,
            });
            emit(cli, &status, |_| {
                println!(
                    "{host}:{} is an HDM (TCP connect in {connect_ms} ms)",
                    cli.port
                );
                println!("  protocol version:     {protocol}");
                println!("  HDM software version: {software}");
                println!("  probe response code:  {} - {note}", id.response_code);
            })
        }
        Err(HdmError::NotHdm { protocol_version }) => {
            let (b0, b1) = protocol_version;
            let detail =
                format!("answered with 0x{b0:02x} 0x{b1:02x}, not the HDM protocol version");
            let status = serde_json::json!({
                "host": host,
                "port": cli.port,
                "reachable": true,
                "responded": true,
                "is_hdm": false,
                "connect_ms": connect_ms,
                "detail": detail,
            });
            emit(cli, &status, |_| {
                println!(
                    "{host}:{} is reachable but is NOT an HDM (TCP connect in {connect_ms} ms)",
                    cli.port
                );
                println!("  {detail} - some other service is on this port.");
            })
        }
        Err(HdmError::Transport(err)) => {
            let detail = format!("{err}");
            let status = serde_json::json!({
                "host": host,
                "port": cli.port,
                "reachable": true,
                "responded": false,
                "is_hdm": false,
                "connect_ms": connect_ms,
                "detail": detail,
            });
            emit(cli, &status, |_| {
                println!(
                    "{host}:{} accepted the connection but did not answer the HDM probe (TCP connect in {connect_ms} ms)",
                    cli.port
                );
                println!("  {detail}");
                println!(
                    "  the port is open but mute - likely a firewall, the wrong port, or not an HDM."
                );
            })
        }
        Err(other) => Err(other).context("probing endpoint"),
    }
}

/// Human note about the response code an unauthenticated probe drew. Both common codes still
/// confirm the endpoint is an HDM; `403` additionally tells the operator their IP needs
/// whitelisting on the device's integration screen before authenticated calls will work.
const fn probe_code_note(code: u16) -> &'static str {
    match ServerErrorKind::from_code(code) {
        ServerErrorKind::UnauthorizedConnection => {
            "this caller's IP is not yet whitelisted on the device (expected on first contact)"
        }
        ServerErrorKind::CryptographicError => {
            "the probe's throwaway payload failed to decrypt (expected for an unauthenticated probe)"
        }
        _ => "unusual for an unauthenticated probe, but the header confirms an HDM",
    }
}

fn operators(cli: &Cli) -> Result<()> {
    let mut c = client(cli)?;
    let response = c
        .list_operators_and_departments()
        .context("listing operators and departments")?;
    emit(cli, &response, format::operators)
}

fn login(cli: &Cli) -> Result<()> {
    with_session(cli, |_c| Ok(()))?;
    let status = serde_json::json!({ "ok": true, "message": "credentials accepted" });
    emit(cli, &status, |_| {
        println!("login ok: credentials accepted and session round-tripped");
    })
}

fn datetime(cli: &Cli) -> Result<()> {
    let response = with_session(cli, |c| c.date_time().context("querying date/time"))?;
    emit(cli, &response, |r| println!("device time: {}", r.dt))
}

fn payment_systems(cli: &Cli) -> Result<()> {
    let response = with_session(cli, |c| {
        c.payment_systems_list().context("listing payment systems")
    })?;
    emit(cli, &response, format::payment_systems)
}

fn sample(cli: &Cli) -> Result<()> {
    if !confirm("Print a sample receipt? This consumes paper.")? {
        bail!("aborted");
    }
    with_session(cli, |c| {
        c.receipt_sample().context("printing sample receipt")
    })?;
    let status = serde_json::json!({ "ok": true, "printed": "sample" });
    emit(cli, &status, |_| println!("sample receipt printed"))
}

fn receipt(cli: &Cli, args: &ReceiptArgs) -> Result<()> {
    let request = build_receipt_request(args)?;

    // `Decimal + Decimal` panics on overflow; fold with `checked_add` so absurd amounts surface as
    // a clean error instead of crashing before the device ever sees the request.
    let Some(total) = [args.card, args.partial, args.prepayment]
        .into_iter()
        .try_fold(args.cash, Decimal::checked_add)
    else {
        bail!("payment amounts are too large to sum");
    };
    let prompt = format!(
        "Print a FISCAL receipt: total {} AMD (cash {} / card {})? This registers a sale.",
        total.round_dp(2),
        args.cash.round_dp(2),
        args.card.round_dp(2)
    );
    if !args.yes && !confirm(&prompt)? {
        bail!("aborted");
    }

    let printed = request.clone();
    let response = with_session(cli, move |c| {
        c.print_receipt(printed).context("printing receipt")
    })?;
    // Render the faithful receipt summary from the request we sent + the response we got back.
    emit(cli, &response, |resp| {
        print!(
            "{}",
            hdm_am::format_receipt(&request, resp).to_plain_text(hdm_am::DEFAULT_WIDTH)
        );
    })
}

fn print_last(cli: &Cli) -> Result<()> {
    with_session(cli, |c| {
        c.print_last_receipt().context("reprinting last receipt")
    })?;
    let status = serde_json::json!({ "ok": true, "printed": "last" });
    emit(cli, &status, |_| println!("last receipt reprinted"))
}

fn report(cli: &Cli, args: &ReportArgs) -> Result<()> {
    if matches!(args.kind, ReportKind::Z)
        && !args.yes
        && !confirm("Print a Z-report? This closes the fiscal day and zeros counters.")?
    {
        bail!("aborted");
    }
    let request = build_fiscal_report_request(args)?;
    with_session(cli, |c| {
        c.fiscal_report(request).context("printing fiscal report")
    })?;
    let label = match args.kind {
        ReportKind::X => "x",
        ReportKind::Z => "z",
    };
    let status = serde_json::json!({ "ok": true, "report": label });
    emit(cli, &status, |_| {
        println!("{} report printed", label.to_uppercase());
    })
}

fn cash(cli: &Cli, args: &CashArgs) -> Result<()> {
    if args.amount <= Decimal::ZERO {
        bail!("--amount must be greater than zero");
    }
    let is_cash_in = matches!(args.direction, CashDirection::In);
    let dir = if is_cash_in { "cash-in" } else { "cash-out" };
    let amount = args.amount.round_dp(2);
    if !args.yes && !confirm(&format!("Record {dir} of {amount} AMD?"))? {
        bail!("aborted");
    }
    let request = CashInOutRequest {
        amount: args.amount,
        is_cash_in,
        cashier_id: args.cashier_id.or(cli.cashier),
        description: args.description.clone(),
    };
    with_session(cli, |c| {
        c.cash_in_out(request).context("recording cash adjustment")
    })?;
    let status = serde_json::json!({ "ok": true, "direction": dir, "amount": amount.to_string() });
    emit(cli, &status, |_| {
        println!("recorded {dir} of {amount} AMD");
    })
}

fn time_sync(cli: &Cli) -> Result<()> {
    with_session(cli, |c| {
        c.hdm_time_sync()
            .context("synchronising with the tax authority")
    })?;
    let status = serde_json::json!({ "ok": true });
    emit(cli, &status, |_| {
        println!("device synchronised with the tax authority");
    })
}

fn emark(cli: &Cli, code: &str) -> Result<()> {
    with_session(cli, |c| c.single_emark(code).context("submitting eMark"))?;
    let status = serde_json::json!({ "ok": true });
    emit(cli, &status, |_| println!("eMark accepted"))
}

fn lookup_receipt(cli: &Cli, args: &LookupReceiptArgs) -> Result<()> {
    let response = with_session(cli, |c| {
        c.get_returnable_receipt(args.receipt_id.clone(), args.crn.clone())
            .context("looking up receipt")
    })?;
    emit(cli, &response, format::returnable_receipt)
}

fn return_receipt(cli: &Cli, args: &ReturnArgs) -> Result<()> {
    let request = build_return_receipt_request(args)?;
    if !args.yes
        && !confirm(&format!(
            "Print a return receipt for ticket {} (crn {})? This registers a refund.",
            args.ticket, args.crn
        ))?
    {
        bail!("aborted");
    }
    let response = with_session(cli, |c| {
        c.print_return_receipt(request)
            .context("printing return receipt")
    })?;
    emit(cli, &response, |r| {
        println!("Return receipt printed:");
        println!("  return seq: {}", r.rseq);
        println!("  fiscal:     {}", r.fiscal);
        println!("  total:      {:.2}", r.total);
        println!("  change:     {:.2}", r.change);
    })
}

fn header_footer(cli: &Cli, args: &HeaderFooterArgs) -> Result<()> {
    let raw = std::fs::read_to_string(&args.file)
        .with_context(|| format!("reading {}", args.file.display()))?;
    let parsed: HeaderFooterFile = serde_json::from_str(&raw)
        .with_context(|| format!("parsing header/footer JSON from {}", args.file.display()))?;
    let request = SetupHeaderFooterRequest {
        headers: parsed.headers,
        footers: parsed.footers,
    };
    with_session(cli, |c| {
        c.setup_header_footer(request)
            .context("configuring header/footer")
    })?;
    let status = serde_json::json!({ "ok": true });
    emit(cli, &status, |_| println!("header/footer configured"))
}

fn logo(cli: &Cli, args: &LogoArgs) -> Result<()> {
    let bytes =
        std::fs::read(&args.image).with_context(|| format!("reading {}", args.image.display()))?;
    let encoded = BASE64.encode(&bytes);
    with_session(cli, |c| {
        c.setup_header_logo(encoded)
            .context("uploading header logo")
    })?;
    let status = serde_json::json!({ "ok": true });
    emit(cli, &status, |_| println!("logo uploaded"))
}

fn build_receipt_request(args: &ReceiptArgs) -> Result<PrintReceiptRequest> {
    let mode = match args.mode {
        ReceiptMode::Simple => PrintMode::Simple,
        ReceiptMode::Products => PrintMode::Products,
        ReceiptMode::Prepayment => PrintMode::Prepayment,
    };

    let items = match (&args.items, args.mode) {
        (Some(path), ReceiptMode::Products) => read_items(path)?,
        (Some(_), _) => bail!("--items is only valid with --mode products"),
        (None, ReceiptMode::Products) => bail!("--mode products requires --items <FILE>"),
        (None, _) => Vec::new(),
    };

    if matches!(args.mode, ReceiptMode::Simple | ReceiptMode::Prepayment) && args.dep.is_none() {
        bail!("--dep is required for simple and prepayment receipts");
    }
    if !matches!(args.mode, ReceiptMode::Products) && !args.e_marks.is_empty() {
        bail!("--emark is only valid with --mode products");
    }

    let (rrn, terminal_id) = if args.use_ext_pos {
        if args.payment_system.is_some() {
            bail!("--payment-system cannot be used with --use-ext-pos");
        }
        let rrn = match &args.rrn {
            Some(rrn) => rrn.clone(),
            None => bail!("--use-ext-pos requires --rrn"),
        };
        let terminal_id = match &args.terminal_id {
            Some(terminal_id) => terminal_id.clone(),
            None => bail!("--use-ext-pos requires --terminal-id"),
        };
        (Some(rrn), Some(terminal_id))
    } else {
        if args.rrn.is_some() || args.terminal_id.is_some() {
            bail!("--rrn and --terminal-id require --use-ext-pos");
        }
        (None, None)
    };

    Ok(PrintReceiptRequest {
        mode,
        paid_amount: args.cash,
        paid_amount_card: args.card,
        partial_amount: args.partial,
        pre_payment_amount: args.prepayment,
        dep: args.dep,
        partner_tin: args.partner_tin.clone(),
        use_ext_pos: args.use_ext_pos,
        payment_system: args.payment_system,
        rrn,
        terminal_id,
        e_marks: args.e_marks.clone(),
        items,
    })
}

fn build_fiscal_report_request(args: &ReportArgs) -> Result<FiscalReportRequest> {
    let filters = [
        args.dept.is_some(),
        args.cashier_id.is_some(),
        args.transaction_type.is_some(),
    ]
    .into_iter()
    .filter(|selected| *selected)
    .count();
    if filters > 1 {
        bail!("--dept, --cashier-id, and --transaction-type are mutually exclusive");
    }

    let kind = match args.kind {
        ReportKind::X => FiscalReportKind::X,
        ReportKind::Z => FiscalReportKind::Z,
    };
    let filter = match (args.dept, args.cashier_id, args.transaction_type) {
        (Some(dept), None, None) => Some(ReportFilter::Department(dept)),
        (None, Some(cashier), None) => Some(ReportFilter::Cashier(cashier)),
        (None, None, Some(transaction_type)) => {
            Some(ReportFilter::TransactionType(transaction_type))
        }
        (None, None, None) => None,
        _ => unreachable!("multiple filters were rejected above"),
    };

    Ok(FiscalReportRequest {
        kind,
        filter,
        start_date: args.start,
        end_date: args.end,
    })
}

fn build_return_receipt_request(args: &ReturnArgs) -> Result<PrintReturnReceiptRequest> {
    let return_item_list = match &args.return_items {
        Some(path) => read_return_items(path)?,
        None => Vec::new(),
    };

    Ok(PrintReturnReceiptRequest {
        crn: args.crn.clone(),
        return_ticket_id: args.ticket,
        cash_amount_for_return: args.cash,
        card_amount_for_return: args.card,
        pre_payment_amount_for_return: args.prepayment,
        rrn: args.rrn.clone(),
        terminal_id: args.terminal_id.clone(),
        e_marks: args.e_marks.clone(),
        return_item_list,
    })
}

/// Load a JSON array of receipt items from a file (for `receipt --mode products`).
fn read_items(path: &std::path::Path) -> Result<Vec<ReceiptItem>> {
    read_json_array(path, "receipt items")
}

/// Load a JSON array of return items from a file (for `return --return-items`).
fn read_return_items(path: &std::path::Path) -> Result<Vec<ReturnItem>> {
    read_json_array(path, "return items")
}

fn read_json_array<T>(path: &std::path::Path, label: &str) -> Result<Vec<T>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading {label} file {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing {label} from {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{
        build_fiscal_report_request, build_receipt_request, build_return_receipt_request,
        probe_code_note,
    };
    use crate::{ReceiptArgs, ReceiptMode, ReportArgs, ReportKind, ReturnArgs};
    use hdm_am::{PrintMode, ReportFilter};
    use rust_decimal::Decimal;
    use std::path::PathBuf;

    #[test]
    fn probe_code_note_maps_known_codes() {
        assert!(probe_code_note(403).contains("whitelisted"));
        assert!(probe_code_note(101).contains("decrypt"));
        assert!(probe_code_note(999).contains("unusual"));
    }

    #[test]
    fn receipt_request_supports_external_pos_and_emarks() {
        let args = ReceiptArgs {
            mode: ReceiptMode::Products,
            cash: Decimal::ZERO,
            card: Decimal::new(1000, 0),
            partial: Decimal::ZERO,
            prepayment: Decimal::ZERO,
            dep: None,
            partner_tin: None,
            payment_system: None,
            use_ext_pos: true,
            rrn: Some("123456789012".to_owned()),
            terminal_id: Some("12345678".to_owned()),
            e_marks: vec!["emark-1".to_owned()],
            items: Some(write_temp_file(
                "receipt-items",
                r#"[{"dep":1,"qty":1,"price":1000,"productCode":"A","productName":"Item","unit":"pcs"}]"#,
            )),
            yes: true,
        };

        let request = build_receipt_request(&args).unwrap();

        assert_eq!(request.mode, PrintMode::Products);
        assert!(request.use_ext_pos);
        assert_eq!(request.rrn.as_deref(), Some("123456789012"));
        assert_eq!(request.terminal_id.as_deref(), Some("12345678"));
        assert_eq!(request.e_marks, ["emark-1"]);
        assert_eq!(request.items.len(), 1);
    }

    #[test]
    fn receipt_request_rejects_external_pos_without_terminal_pair() {
        let args = ReceiptArgs {
            mode: ReceiptMode::Simple,
            cash: Decimal::new(1000, 0),
            card: Decimal::ZERO,
            partial: Decimal::ZERO,
            prepayment: Decimal::ZERO,
            dep: Some(1),
            partner_tin: None,
            payment_system: None,
            use_ext_pos: true,
            rrn: Some("123456789012".to_owned()),
            terminal_id: None,
            e_marks: Vec::new(),
            items: None,
            yes: true,
        };

        let err = build_receipt_request(&args).unwrap_err();

        assert!(err.to_string().contains("--terminal-id"));
    }

    #[test]
    fn fiscal_report_request_supports_transaction_filter() {
        let args = ReportArgs {
            kind: ReportKind::X,
            dept: None,
            cashier_id: None,
            transaction_type: Some(1),
            start: 10,
            end: 20,
            yes: true,
        };

        let request = build_fiscal_report_request(&args).unwrap();

        assert_eq!(request.filter, Some(ReportFilter::TransactionType(1)));
        assert_eq!(request.start_date, 10);
        assert_eq!(request.end_date, 20);
    }

    #[test]
    fn fiscal_report_request_rejects_multiple_filters() {
        let args = ReportArgs {
            kind: ReportKind::X,
            dept: Some(1),
            cashier_id: None,
            transaction_type: Some(1),
            start: 0,
            end: 0,
            yes: true,
        };

        let err = build_fiscal_report_request(&args).unwrap_err();

        assert!(err.to_string().contains("mutually exclusive"));
    }

    #[test]
    fn return_request_supports_items_and_emarks() {
        let args = ReturnArgs {
            crn: "51815332".to_owned(),
            ticket: 42,
            cash: Some(Decimal::new(500, 0)),
            card: None,
            prepayment: None,
            rrn: Some("123456789012".to_owned()),
            terminal_id: Some("12345678".to_owned()),
            e_marks: vec!["emark-1".to_owned(), "emark-2".to_owned()],
            return_items: Some(write_temp_file(
                "return-items",
                r#"[{"rpid":100,"quantity":1.5}]"#,
            )),
            yes: true,
        };

        let request = build_return_receipt_request(&args).unwrap();

        assert_eq!(request.e_marks, ["emark-1", "emark-2"]);
        assert_eq!(request.return_item_list.len(), 1);
        assert_eq!(request.return_item_list[0].rpid, 100);
        assert_eq!(request.return_item_list[0].quantity, Decimal::new(15, 1));
    }

    fn write_temp_file(name: &str, contents: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("hdm-am-cli-{name}-{}.json", std::process::id()));
        std::fs::write(&path, contents).unwrap();
        path
    }
}
