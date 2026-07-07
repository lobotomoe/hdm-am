//! The device-access boundary.
//!
//! Handlers depend on the [`Device`] trait, not on TCP or crypto. The production [`TcpDevice`] runs
//! the proven `connect -> login -> op -> logout` lifecycle (mirroring the CLI's `with_session`) on
//! a fresh socket per call — a fresh login is a fresh server-side session, so no sequence state has
//! to persist. Tests substitute a fake `Device` returning canned results, so envelope/auth/CORS and
//! error mapping are exercised without a real device.

use std::net::{TcpStream, ToSocketAddrs};

use hdm_am::{
    CashInOutRequest, Client, DateTimeResponse, EmptyResponse, FiscalReportRequest,
    GetReturnableReceiptRequest, InMemorySeq, ListOpsAndDepsResponse, PaymentSystemsListResponse,
    PrintReceiptRequest, PrintReturnReceiptRequest, ReceiptResponse, ReturnReceiptResponse,
    ReturnableReceiptResponse, SetupHeaderFooterRequest, SetupHeaderLogoRequest,
    SingleEmarkRequest,
};

use crate::config::{EndpointConn, PasswordConn, SessionConn};

/// One method per bridge operation. `Send + Sync + 'static` so it can live in `Arc` shared state
/// and be moved into a blocking worker.
pub trait Device: Send + Sync + 'static {
    fn operators(&self, conn: &PasswordConn) -> Result<ListOpsAndDepsResponse, hdm_am::Error>;
    fn verify_login(&self, conn: &SessionConn) -> Result<(), hdm_am::Error>;
    fn print_receipt(
        &self,
        conn: &SessionConn,
        req: PrintReceiptRequest,
    ) -> Result<ReceiptResponse, hdm_am::Error>;
    fn print_last_receipt(&self, conn: &SessionConn) -> Result<EmptyResponse, hdm_am::Error>;
    fn lookup_receipt(
        &self,
        conn: &SessionConn,
        req: GetReturnableReceiptRequest,
    ) -> Result<ReturnableReceiptResponse, hdm_am::Error>;
    fn print_return(
        &self,
        conn: &SessionConn,
        req: PrintReturnReceiptRequest,
    ) -> Result<ReturnReceiptResponse, hdm_am::Error>;
    fn fiscal_report(
        &self,
        conn: &SessionConn,
        req: FiscalReportRequest,
    ) -> Result<EmptyResponse, hdm_am::Error>;
    fn cash_in_out(
        &self,
        conn: &SessionConn,
        req: CashInOutRequest,
    ) -> Result<EmptyResponse, hdm_am::Error>;
    fn date_time(&self, conn: &SessionConn) -> Result<DateTimeResponse, hdm_am::Error>;
    fn time_sync(&self, conn: &SessionConn) -> Result<EmptyResponse, hdm_am::Error>;
    fn payment_systems(
        &self,
        conn: &SessionConn,
    ) -> Result<PaymentSystemsListResponse, hdm_am::Error>;
    fn single_emark(
        &self,
        conn: &SessionConn,
        req: SingleEmarkRequest,
    ) -> Result<EmptyResponse, hdm_am::Error>;
    fn receipt_sample(&self, conn: &SessionConn) -> Result<EmptyResponse, hdm_am::Error>;
    fn header_footer(
        &self,
        conn: &SessionConn,
        req: SetupHeaderFooterRequest,
    ) -> Result<EmptyResponse, hdm_am::Error>;
    fn header_logo(
        &self,
        conn: &SessionConn,
        req: SetupHeaderLogoRequest,
    ) -> Result<EmptyResponse, hdm_am::Error>;
}

/// The production device: a real TCP socket per call.
pub struct TcpDevice;

type TcpHdmClient = Client<TcpStream, InMemorySeq>;

impl TcpDevice {
    fn dial(endpoint: &EndpointConn) -> Result<TcpStream, hdm_am::Error> {
        let addr = (endpoint.host.as_str(), endpoint.port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "{}:{} resolved to no addresses",
                        endpoint.host, endpoint.port
                    ),
                )
            })?;
        let stream = TcpStream::connect_timeout(&addr, endpoint.timeout)?;
        stream.set_read_timeout(Some(endpoint.timeout))?;
        stream.set_write_timeout(Some(endpoint.timeout))?;
        Ok(stream)
    }

    /// Open a client over a fresh socket for a password-key operation (no login).
    fn with_password<R>(
        conn: &PasswordConn,
        op: impl FnOnce(&mut TcpHdmClient) -> Result<R, hdm_am::Error>,
    ) -> Result<R, hdm_am::Error> {
        let stream = Self::dial(&conn.endpoint)?;
        let mut client = Client::new(stream, conn.password.clone(), InMemorySeq::default());
        op(&mut client)
    }

    /// Connect, log in, run `op`, then log out — even if `op` fails. A logout failure is logged but
    /// never masks the operation's own outcome.
    fn with_session<R>(
        conn: &SessionConn,
        op: impl FnOnce(&mut TcpHdmClient) -> Result<R, hdm_am::Error>,
    ) -> Result<R, hdm_am::Error> {
        let stream = Self::dial(&conn.endpoint)?;
        let mut client = Client::new(stream, conn.password.clone(), InMemorySeq::default());
        client.login(conn.cashier, conn.pin.clone())?;
        let result = op(&mut client);
        if let Err(err) = client.logout() {
            log::warn!("hdm-bridge: logout failed: {err}");
        }
        result
    }
}

impl Device for TcpDevice {
    fn operators(&self, conn: &PasswordConn) -> Result<ListOpsAndDepsResponse, hdm_am::Error> {
        Self::with_password(conn, TcpHdmClient::list_operators_and_departments)
    }

    fn verify_login(&self, conn: &SessionConn) -> Result<(), hdm_am::Error> {
        // `with_session` already logs in and out; the closure just confirms the session opened.
        Self::with_session(conn, |_client| Ok(()))
    }

    fn print_receipt(
        &self,
        conn: &SessionConn,
        req: PrintReceiptRequest,
    ) -> Result<ReceiptResponse, hdm_am::Error> {
        Self::with_session(conn, move |client| client.print_receipt(req))
    }

    fn print_last_receipt(&self, conn: &SessionConn) -> Result<EmptyResponse, hdm_am::Error> {
        Self::with_session(conn, TcpHdmClient::print_last_receipt)
    }

    fn lookup_receipt(
        &self,
        conn: &SessionConn,
        req: GetReturnableReceiptRequest,
    ) -> Result<ReturnableReceiptResponse, hdm_am::Error> {
        Self::with_session(conn, move |client| {
            client.get_returnable_receipt(req.receipt_id, req.crn)
        })
    }

    fn print_return(
        &self,
        conn: &SessionConn,
        req: PrintReturnReceiptRequest,
    ) -> Result<ReturnReceiptResponse, hdm_am::Error> {
        Self::with_session(conn, move |client| client.print_return_receipt(req))
    }

    fn fiscal_report(
        &self,
        conn: &SessionConn,
        req: FiscalReportRequest,
    ) -> Result<EmptyResponse, hdm_am::Error> {
        Self::with_session(conn, move |client| client.fiscal_report(req))
    }

    fn cash_in_out(
        &self,
        conn: &SessionConn,
        req: CashInOutRequest,
    ) -> Result<EmptyResponse, hdm_am::Error> {
        Self::with_session(conn, move |client| client.cash_in_out(req))
    }

    fn date_time(&self, conn: &SessionConn) -> Result<DateTimeResponse, hdm_am::Error> {
        Self::with_session(conn, TcpHdmClient::date_time)
    }

    fn time_sync(&self, conn: &SessionConn) -> Result<EmptyResponse, hdm_am::Error> {
        Self::with_session(conn, TcpHdmClient::hdm_time_sync)
    }

    fn payment_systems(
        &self,
        conn: &SessionConn,
    ) -> Result<PaymentSystemsListResponse, hdm_am::Error> {
        Self::with_session(conn, TcpHdmClient::payment_systems_list)
    }

    fn single_emark(
        &self,
        conn: &SessionConn,
        req: SingleEmarkRequest,
    ) -> Result<EmptyResponse, hdm_am::Error> {
        Self::with_session(conn, move |client| client.single_emark(req.e_mark))
    }

    fn receipt_sample(&self, conn: &SessionConn) -> Result<EmptyResponse, hdm_am::Error> {
        Self::with_session(conn, TcpHdmClient::receipt_sample)
    }

    fn header_footer(
        &self,
        conn: &SessionConn,
        req: SetupHeaderFooterRequest,
    ) -> Result<EmptyResponse, hdm_am::Error> {
        Self::with_session(conn, move |client| client.setup_header_footer(req))
    }

    fn header_logo(
        &self,
        conn: &SessionConn,
        req: SetupHeaderLogoRequest,
    ) -> Result<EmptyResponse, hdm_am::Error> {
        Self::with_session(conn, move |client| {
            client.setup_header_logo(req.header_logo)
        })
    }
}
