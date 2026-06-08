use std::net::{TcpStream, ToSocketAddrs};
use std::thread;
use std::time::Instant;

use hdm_am::{Client, Error as HdmError, FiscalReportKind, InMemorySeq, identify};
use slint::{ComponentHandle, SharedString, Weak};

use crate::generated::MainWindow;
use crate::validation::{ConnectionInput, ConnectionSettings, OperationInputs};
use crate::{format as ui_format, validation};

#[derive(Clone, Copy)]
enum Action {
    Probe,
    Operators,
    VerifyLogin,
    Receipt,
    PrintLast,
    LookupReceipt,
    HeaderFooter,
    Logo,
    Report,
    Return,
    Cash,
    Datetime,
    Sample,
    TimeSync,
    PaymentSystems,
    Emark,
}

impl Action {
    const fn label(self) -> &'static str {
        match self {
            Self::Probe => "Probe",
            Self::Operators => "Operators",
            Self::VerifyLogin => "Verify login",
            Self::Receipt => "Receipt",
            Self::PrintLast => "Print last",
            Self::LookupReceipt => "Lookup receipt",
            Self::HeaderFooter => "Header/footer",
            Self::Logo => "Logo",
            Self::Report => "Report",
            Self::Return => "Return",
            Self::Cash => "Cash",
            Self::Datetime => "Datetime",
            Self::Sample => "Sample",
            Self::TimeSync => "Time sync",
            Self::PaymentSystems => "Payment systems",
            Self::Emark => "eMark",
        }
    }

    const fn needs_password(self) -> bool {
        !matches!(self, Self::Probe)
    }

    const fn needs_session(self) -> bool {
        !matches!(self, Self::Probe | Self::Operators)
    }

    fn requires_confirmation(self, inputs: &OperationInputs) -> bool {
        match self {
            Self::Receipt
            | Self::PrintLast
            | Self::HeaderFooter
            | Self::Logo
            | Self::Return
            | Self::Cash
            | Self::Sample
            | Self::TimeSync
            | Self::Emark => true,
            Self::Report => inputs.report_kind.trim().eq_ignore_ascii_case("z"),
            Self::Probe
            | Self::Operators
            | Self::VerifyLogin
            | Self::LookupReceipt
            | Self::Datetime
            | Self::PaymentSystems => false,
        }
    }
}

/// Run the GUI application.
///
/// # Errors
/// Returns a Slint platform error if the native windowing backend cannot be initialised.
pub fn run() -> Result<(), slint::PlatformError> {
    let window = MainWindow::new()?;
    wire_callbacks(&window);
    window.run()
}

fn wire_callbacks(window: &MainWindow) {
    let weak = window.as_weak();
    window.on_probe_requested(move || start_action(&weak, Action::Probe));

    let weak = window.as_weak();
    window.on_operators_requested(move || start_action(&weak, Action::Operators));

    let weak = window.as_weak();
    window.on_login_requested(move || start_action(&weak, Action::VerifyLogin));

    let weak = window.as_weak();
    window.on_receipt_requested(move || start_action(&weak, Action::Receipt));

    let weak = window.as_weak();
    window.on_print_last_requested(move || start_action(&weak, Action::PrintLast));

    let weak = window.as_weak();
    window.on_lookup_requested(move || start_action(&weak, Action::LookupReceipt));

    let weak = window.as_weak();
    window.on_header_footer_requested(move || start_action(&weak, Action::HeaderFooter));

    let weak = window.as_weak();
    window.on_logo_requested(move || start_action(&weak, Action::Logo));

    let weak = window.as_weak();
    window.on_report_requested(move || start_action(&weak, Action::Report));

    let weak = window.as_weak();
    window.on_return_requested(move || start_action(&weak, Action::Return));

    let weak = window.as_weak();
    window.on_cash_requested(move || start_action(&weak, Action::Cash));

    let weak = window.as_weak();
    window.on_datetime_requested(move || start_action(&weak, Action::Datetime));

    let weak = window.as_weak();
    window.on_sample_requested(move || start_action(&weak, Action::Sample));

    let weak = window.as_weak();
    window.on_time_sync_requested(move || start_action(&weak, Action::TimeSync));

    let weak = window.as_weak();
    window.on_payment_systems_requested(move || start_action(&weak, Action::PaymentSystems));

    let weak = window.as_weak();
    window.on_emark_requested(move || start_action(&weak, Action::Emark));

    let weak = window.as_weak();
    window.on_privacy_requested(move || show_privacy(&weak));
}

fn start_action(weak: &Weak<MainWindow>, action: Action) {
    let Some(window) = weak.upgrade() else {
        return;
    };

    let inputs = read_inputs(&window);
    let demo_mode = window.get_demo_mode();
    if !demo_mode && action.requires_confirmation(&inputs) && !inputs.confirm_operation {
        set_result(
            &window,
            "Input error",
            "This operation changes device state or prints paper. Enable \"Confirm side effect\" and retry.",
            "Confirmation required.",
            false,
        );
        return;
    }

    if demo_mode {
        set_result(
            &window,
            "Ready",
            &demo_result(action),
            "Demo operation completed.",
            false,
        );
        window.set_confirm_operation(false);
        return;
    }

    let settings = match read_settings(&window, action) {
        Ok(settings) => settings,
        Err(message) => {
            set_result(
                &window,
                "Input error",
                &message,
                "Fix connection inputs and retry.",
                false,
            );
            return;
        }
    };

    window.set_busy(true);
    window.set_status(format!("{label} running...", label = action.label()).into());
    window.set_detail("Waiting for HDM response.".into());
    window.set_last_result("".into());

    let weak_for_worker = weak.clone();
    thread::spawn(move || {
        // A panic inside run_action (a future regression, a library debug_assert, an allocation
        // failure on a huge input file) must not strand `busy = true` and freeze every button —
        // each one is gated on `enabled: !root.busy`. Catch it so the event-loop callback below
        // always runs and clears the flag.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_action(action, &settings, &inputs)
        }))
        .unwrap_or_else(|_| {
            Err("Internal error: the operation panicked. Check logs and retry.".to_owned())
        });
        let invoke_result = slint::invoke_from_event_loop(move || {
            if let Some(window) = weak_for_worker.upgrade() {
                match outcome {
                    Ok(message) => {
                        set_result(&window, "Ready", &message, "Operation completed.", false);
                    }
                    Err(message) => {
                        set_result(&window, "Error", &message, "Operation failed.", false);
                    }
                }
                window.set_confirm_operation(false);
            }
        });

        if let Err(err) = invoke_result {
            log::warn!("failed to report GUI action result: {err}");
        }
    });
}

fn show_privacy(weak: &Weak<MainWindow>) {
    let Some(window) = weak.upgrade() else {
        return;
    };
    set_result(
        &window,
        "Ready",
        privacy_summary(),
        "Privacy information.",
        false,
    );
}

fn read_settings(window: &MainWindow, action: Action) -> Result<ConnectionSettings, String> {
    validation::connection_settings(&ConnectionInput {
        host: window.get_host().as_str(),
        port: window.get_port().as_str(),
        timeout_seconds: window.get_timeout_seconds().as_str(),
        password: window.get_password().as_str(),
        cashier: window.get_cashier().as_str(),
        pin: window.get_pin().as_str(),
        needs_password: action.needs_password(),
        needs_session: action.needs_session(),
    })
}

fn read_inputs(window: &MainWindow) -> OperationInputs {
    OperationInputs {
        receipt_mode: window.get_receipt_mode().to_string(),
        amount: window.get_amount().to_string(),
        card_amount: window.get_card_amount().to_string(),
        partial_amount: window.get_partial_amount().to_string(),
        prepayment_amount: window.get_prepayment_amount().to_string(),
        department: window.get_department().to_string(),
        partner_tin: window.get_partner_tin().to_string(),
        payment_system: window.get_payment_system().to_string(),
        use_ext_pos: window.get_use_ext_pos(),
        rrn: window.get_rrn().to_string(),
        terminal_id: window.get_terminal_id().to_string(),
        crn: window.get_crn().to_string(),
        receipt_id: window.get_receipt_id().to_string(),
        ticket: window.get_ticket().to_string(),
        emarks: window.get_emarks().to_string(),
        json_path: window.get_json_path().to_string(),
        logo_path: window.get_logo_path().to_string(),
        description: window.get_description().to_string(),
        cash_in: window.get_cash_in(),
        report_kind: window.get_report_kind().to_string(),
        report_filter_kind: window.get_report_filter_kind().to_string(),
        report_filter_value: window.get_report_filter_value().to_string(),
        report_start: window.get_report_start().to_string(),
        report_end: window.get_report_end().to_string(),
        confirm_operation: window.get_confirm_operation(),
    }
}

fn demo_result(action: Action) -> String {
    let suffix = "\n\nDemo mode: no network request was sent and no fiscal data was registered.";
    let body = match action {
        Action::Probe => {
            "10.0.0.5:1025 is an HDM\nTCP connect: 4 ms\nProtocol: 0.7\nSoftware: 1.1.0\nProbe response code: 200"
        }
        Action::Operators => {
            "Operators: 2\nDepartments: 2\n\nOperators\n  [1] Administrator  departments: [1, 2]\n  [3] Cashier  departments: [1]\n\nDepartments\n  [1] Sales\n  [2] Service"
        }
        Action::VerifyLogin => "Credentials accepted.",
        Action::Receipt => {
            "Fiscal receipt printed\n  fiscal number: 12345678\n  receipt seq:   42\n  reg number:    51815332\n  serial:        HDM-DEMO-001\n  total:         10.00\n  change:        0.00\n  verification:  DEMO-VERIFY"
        }
        Action::PrintLast => "Last receipt reprinted.",
        Action::LookupReceipt => {
            "Receipt lookup\n  receipt seq: 42\n  cashier id:  3\n  sale type:   sale\n  total:       10.00\n  cash:        10.00\n  card:        0.00\n  eMarks:      0\n  items:       1\n\nItems\n  [1001] Demo item  qty 1 x 10.00"
        }
        Action::HeaderFooter => "Header/footer configured.",
        Action::Logo => "Logo uploaded.",
        Action::Report => "X-report printed.",
        Action::Return => {
            "Return receipt printed\n  return seq: 43\n  fiscal:     12345679\n  reg number: 51815332\n  total:      10.00\n  change:     0.00\n  verification: DEMO-RETURN"
        }
        Action::Cash => "Recorded cash-in.",
        Action::Datetime => "Device time: 2026-06-08T15:40:00+04:00",
        Action::Sample => "Sample receipt printed.",
        Action::TimeSync => "Device synchronised with the tax authority.",
        Action::PaymentSystems => "Payment systems:\n  [1] ArCa\n  [2] Visa/Mastercard",
        Action::Emark => "eMark accepted.",
    };
    format!("{body}{suffix}")
}

const fn privacy_summary() -> &'static str {
    "Privacy policy\n\
HDM Tester does not use analytics, ads, tracking, crash reporting, or developer-operated servers.\n\
Connection settings, HDM password, cashier PIN, receipt data, eMarks, JSON files, and BMP logo files are used only on the device running the app to send the selected request to the HDM address entered by the user.\n\
The app does not intentionally persist HDM credentials, fiscal responses, or receipt payloads.\n\
Full policy: https://github.com/lobotomoe/hdm-am/blob/main/PRIVACY.md"
}

fn run_action(
    action: Action,
    settings: &ConnectionSettings,
    inputs: &OperationInputs,
) -> Result<String, String> {
    match action {
        Action::Probe => probe(settings),
        Action::Operators => operators(settings),
        Action::VerifyLogin => verify_login(settings),
        Action::Receipt => receipt(settings, inputs),
        Action::PrintLast => print_last(settings),
        Action::LookupReceipt => lookup_receipt(settings, inputs),
        Action::HeaderFooter => header_footer(settings, inputs),
        Action::Logo => logo(settings, inputs),
        Action::Report => report(settings, inputs),
        Action::Return => return_receipt(settings, inputs),
        Action::Cash => cash(settings, inputs),
        Action::Datetime => datetime(settings),
        Action::Sample => sample(settings),
        Action::TimeSync => time_sync(settings),
        Action::PaymentSystems => payment_systems(settings),
        Action::Emark => emark(settings, inputs),
    }
}

fn probe(settings: &ConnectionSettings) -> Result<String, String> {
    let started = Instant::now();
    let mut stream = connect(settings)?;
    let connect_ms = started.elapsed().as_millis();

    match identify(&mut stream) {
        Ok(id) => {
            let protocol = format!("{}.{}", id.protocol_version.0, id.protocol_version.1);
            let software = format!(
                "{}.{}.{}",
                id.software_version.0, id.software_version.1, id.software_version.2
            );
            Ok(format!(
                "{}:{} is an HDM\nTCP connect: {connect_ms} ms\nProtocol: {protocol}\nSoftware: {software}\nProbe response code: {}",
                settings.host, settings.port, id.response_code
            ))
        }
        Err(HdmError::NotHdm { protocol_version }) => {
            let (major, minor) = protocol_version;
            Err(format!(
                "{}:{} is reachable but is not an HDM.\nReported protocol bytes: 0x{major:02x} 0x{minor:02x}",
                settings.host, settings.port
            ))
        }
        Err(err) => Err(ui_format::hdm_error("probing endpoint", &err)),
    }
}

fn operators(settings: &ConnectionSettings) -> Result<String, String> {
    let mut client = client(settings)?;
    let response = client
        .list_operators_and_departments()
        .map_err(|err| ui_format::hdm_error("listing operators and departments", &err))?;
    Ok(ui_format::operators(response))
}

fn verify_login(settings: &ConnectionSettings) -> Result<String, String> {
    with_session(settings, |_client| Ok("Credentials accepted.".to_owned()))
}

fn receipt(settings: &ConnectionSettings, inputs: &OperationInputs) -> Result<String, String> {
    let request = validation::build_receipt_request(inputs)?;
    with_session(settings, |client| {
        let response = client
            .print_receipt(request)
            .map_err(|err| ui_format::hdm_error("printing receipt", &err))?;
        Ok(ui_format::receipt(&response))
    })
}

fn print_last(settings: &ConnectionSettings) -> Result<String, String> {
    with_session(settings, |client| {
        client
            .print_last_receipt()
            .map_err(|err| ui_format::hdm_error("reprinting last receipt", &err))?;
        Ok("Last receipt reprinted.".to_owned())
    })
}

fn lookup_receipt(
    settings: &ConnectionSettings,
    inputs: &OperationInputs,
) -> Result<String, String> {
    let (receipt_id, crn) = validation::lookup_args(inputs)?;
    with_session(settings, |client| {
        let response = client
            .get_returnable_receipt(receipt_id, crn)
            .map_err(|err| ui_format::hdm_error("looking up receipt", &err))?;
        Ok(ui_format::returnable_receipt(&response))
    })
}

fn header_footer(
    settings: &ConnectionSettings,
    inputs: &OperationInputs,
) -> Result<String, String> {
    let request = validation::build_header_footer_request(inputs)?;
    with_session(settings, |client| {
        client
            .setup_header_footer(request)
            .map_err(|err| ui_format::hdm_error("configuring header/footer", &err))?;
        Ok("Header/footer configured.".to_owned())
    })
}

fn logo(settings: &ConnectionSettings, inputs: &OperationInputs) -> Result<String, String> {
    let encoded = validation::read_logo_base64(inputs)?;
    with_session(settings, |client| {
        client
            .setup_header_logo(encoded)
            .map_err(|err| ui_format::hdm_error("uploading logo", &err))?;
        Ok("Logo uploaded.".to_owned())
    })
}

fn report(settings: &ConnectionSettings, inputs: &OperationInputs) -> Result<String, String> {
    let request = validation::build_report_request(inputs)?;
    let label = match request.kind {
        FiscalReportKind::X => "X",
        FiscalReportKind::Z => "Z",
    };
    with_session(settings, |client| {
        client
            .fiscal_report(request)
            .map_err(|err| ui_format::hdm_error("printing report", &err))?;
        Ok(format!("{label}-report printed."))
    })
}

fn return_receipt(
    settings: &ConnectionSettings,
    inputs: &OperationInputs,
) -> Result<String, String> {
    let request = validation::build_return_request(inputs)?;
    with_session(settings, |client| {
        let response = client
            .print_return_receipt(request)
            .map_err(|err| ui_format::hdm_error("printing return receipt", &err))?;
        Ok(ui_format::return_receipt(&response))
    })
}

fn cash(settings: &ConnectionSettings, inputs: &OperationInputs) -> Result<String, String> {
    let request = validation::build_cash_request(inputs, settings.cashier)?;
    let label = if inputs.cash_in {
        "cash-in"
    } else {
        "cash-out"
    };
    with_session(settings, |client| {
        client
            .cash_in_out(request)
            .map_err(|err| ui_format::hdm_error("recording cash operation", &err))?;
        Ok(format!("Recorded {label}."))
    })
}

fn datetime(settings: &ConnectionSettings) -> Result<String, String> {
    with_session(settings, |client| {
        let response = client
            .date_time()
            .map_err(|err| ui_format::hdm_error("querying date/time", &err))?;
        Ok(format!("Device time: {}", response.dt))
    })
}

fn sample(settings: &ConnectionSettings) -> Result<String, String> {
    with_session(settings, |client| {
        client
            .receipt_sample()
            .map_err(|err| ui_format::hdm_error("printing sample receipt", &err))?;
        Ok("Sample receipt printed.".to_owned())
    })
}

fn time_sync(settings: &ConnectionSettings) -> Result<String, String> {
    with_session(settings, |client| {
        client
            .hdm_time_sync()
            .map_err(|err| ui_format::hdm_error("synchronising HDM time", &err))?;
        Ok("Device synchronised with the tax authority.".to_owned())
    })
}

fn payment_systems(settings: &ConnectionSettings) -> Result<String, String> {
    with_session(settings, |client| {
        let response = client
            .payment_systems_list()
            .map_err(|err| ui_format::hdm_error("listing payment systems", &err))?;
        Ok(ui_format::payment_systems(&response))
    })
}

fn emark(settings: &ConnectionSettings, inputs: &OperationInputs) -> Result<String, String> {
    let emark = validation::single_emark(inputs)?;
    with_session(settings, |client| {
        client
            .single_emark(emark)
            .map_err(|err| ui_format::hdm_error("submitting eMark", &err))?;
        Ok("eMark accepted.".to_owned())
    })
}

fn with_session(
    settings: &ConnectionSettings,
    op: impl FnOnce(&mut Client<TcpStream, InMemorySeq>) -> Result<String, String>,
) -> Result<String, String> {
    let mut client = client(settings)?;
    client
        .login(settings.cashier, settings.pin.clone())
        .map_err(|err| ui_format::hdm_error("logging in", &err))?;

    let result = op(&mut client);

    if let Err(err) = client.logout() {
        log::warn!("logout failed: {err}");
    }

    result
}

fn client(settings: &ConnectionSettings) -> Result<Client<TcpStream, InMemorySeq>, String> {
    let stream = connect(settings)?;
    Ok(Client::new(
        stream,
        settings.password.clone(),
        InMemorySeq::default(),
    ))
}

fn connect(settings: &ConnectionSettings) -> Result<TcpStream, String> {
    let addr = (settings.host.as_str(), settings.port)
        .to_socket_addrs()
        .map_err(|err| {
            format!(
                "Resolving {}:{} failed: {err}",
                settings.host, settings.port
            )
        })?
        .next()
        .ok_or_else(|| {
            format!(
                "{}:{} resolved to no addresses.",
                settings.host, settings.port
            )
        })?;

    let stream = TcpStream::connect_timeout(&addr, settings.timeout).map_err(|err| {
        format!(
            "Connecting to {addr} failed after {}s: {err}",
            settings.timeout.as_secs()
        )
    })?;
    stream
        .set_read_timeout(Some(settings.timeout))
        .map_err(|err| format!("Setting read timeout failed: {err}"))?;
    stream
        .set_write_timeout(Some(settings.timeout))
        .map_err(|err| format!("Setting write timeout failed: {err}"))?;
    Ok(stream)
}

fn set_result(window: &MainWindow, status: &str, result: &str, detail: &str, busy: bool) {
    window.set_status(SharedString::from(status));
    window.set_last_result(SharedString::from(result));
    window.set_detail(SharedString::from(detail));
    window.set_busy(busy);
}
