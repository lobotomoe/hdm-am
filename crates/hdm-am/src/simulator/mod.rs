//! A simulated HDM: a TCP server that speaks the device side of the protocol.
//!
//! For developing and testing an integration with no cash register on the desk. It frames,
//! encrypts and sequences exactly as [`crate::Client`] expects, keeps the receipts it registered
//! so they can be looked up and returned, and answers the device's own response codes for the
//! mistakes an integration is likely to make (a wrong PIN, an unknown department, a receipt
//! returned twice).
//!
//! It is not a tax device and not a reference for one. Amounts are computed the plain way
//! (quantity × price, less the discount, rounded to two decimals), nothing is sent to the tax
//! authority, and the access list of allowed addresses is not modelled. What a real register
//! does with an edge case is settled on a real register.
//!
//! ```no_run
//! use hdm_am::simulator::{Simulator, SimulatorConfig};
//! use std::net::TcpListener;
//!
//! let simulator = Simulator::new(SimulatorConfig::default());
//! let listener = TcpListener::bind("127.0.0.1:9000")?;
//! simulator.serve(&listener)?;
//! # Ok::<(), std::io::Error>(())
//! ```

mod device;
mod totals;

#[cfg(test)]
mod tests;

use crate::operations::{DepartmentInfo, TaxationKind};
use crate::wire::{MAGIC, RESPONSE_CODE_OK};
use device::Device;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Protocol version the simulator reports in its response header.
const REPORTED_PROTOCOL: [u8; 2] = [0x00, 0x07];
/// Firmware version the simulator reports in its response header.
const REPORTED_SOFTWARE: [u8; 3] = [0, 0, 1];
/// Request header: magic, framing version, operation code, a reserved byte, payload length.
const REQUEST_HEADER_LEN: usize = MAGIC.len() + 2 + 1 + 1 + 2;

/// An operator the simulated device knows.
#[derive(Debug, Clone)]
pub struct SimulatedOperator {
    /// Operator id, as op 1 lists it and op 2 takes it.
    pub id: u32,
    /// Display name.
    pub name: String,
    /// The PIN op 2 accepts for this operator.
    pub pin: String,
}

/// What the simulated device is set up as.
#[derive(Debug, Clone)]
pub struct SimulatorConfig {
    /// The HDM access password.
    pub password: String,
    /// Operators that may log in.
    pub operators: Vec<SimulatedOperator>,
    /// Departments a receipt may be registered under.
    pub departments: Vec<DepartmentInfo>,
    /// Registration number printed on every receipt.
    pub crn: String,
    /// Hardware serial number.
    pub serial_number: String,
    /// Taxpayer TIN.
    pub tin: String,
    /// Taxpayer legal name.
    pub taxpayer: String,
    /// Taxpayer address.
    pub address: String,
}

impl Default for SimulatorConfig {
    /// One operator (id 1, PIN `1234`), one VAT department (id 1), password `simulator`.
    fn default() -> Self {
        Self {
            password: "simulator".to_owned(),
            operators: vec![SimulatedOperator {
                id: 1,
                name: "Operator 1".to_owned(),
                pin: "1234".to_owned(),
            }],
            departments: vec![DepartmentInfo {
                id: 1,
                name: "Department 1".to_owned(),
                kind: TaxationKind::VatTaxable,
            }],
            crn: "00000001".to_owned(),
            serial_number: "SIMULATOR0000001".to_owned(),
            tin: "00000000".to_owned(),
            taxpayer: "Simulated taxpayer".to_owned(),
            address: "Simulated address".to_owned(),
        }
    }
}

/// Something the simulated device is told to get wrong, once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// Answer the next session operation with this response code and do nothing else: a paper
    /// jam is `145`, no link to the tax-authority server `502`.
    Refuse(u16),
    /// Carry out the next session operation and close the connection without answering: the
    /// receipt is registered and the caller never learns it.
    LoseAnswer,
}

/// What one request comes to.
struct Reply {
    code: u16,
    /// Already encrypted. Empty for every code but 200.
    payload: Vec<u8>,
    /// Close the connection instead of sending this.
    lost: bool,
}

impl Reply {
    const fn refused(code: u16) -> Self {
        Self {
            code,
            payload: Vec::new(),
            lost: false,
        }
    }
}

/// A simulated HDM. Cheap to clone: every clone is the same device.
#[derive(Clone)]
pub struct Simulator {
    device: Arc<Mutex<Device>>,
}

impl Simulator {
    /// A device set up as `config`, with nothing registered yet.
    #[must_use]
    pub fn new(config: SimulatorConfig) -> Self {
        Self {
            device: Arc::new(Mutex::new(Device::new(config))),
        }
    }

    /// Get the next session operation wrong in the given way.
    pub fn inject(&self, fault: Fault) {
        self.device().fault = Some(fault);
    }

    /// How many receipts, returns included, the device has registered.
    #[must_use]
    pub fn receipts_registered(&self) -> usize {
        self.device().receipts.len()
    }

    /// Serve connections from `listener` until accepting fails. One thread per connection.
    ///
    /// # Errors
    /// Returns the error of the `accept` that failed.
    pub fn serve(&self, listener: &TcpListener) -> io::Result<()> {
        loop {
            let (stream, peer) = listener.accept()?;
            let simulator = self.clone();
            std::thread::spawn(move || {
                if let Err(error) = simulator.serve_connection(stream) {
                    log::debug!("connection from {peer} ended: {error}");
                }
            });
        }
    }

    /// Answer requests on one connection until the peer closes it.
    fn serve_connection(&self, mut stream: TcpStream) -> io::Result<()> {
        loop {
            let mut header = [0u8; REQUEST_HEADER_LEN];
            match stream.read_exact(&mut header) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(error) => return Err(error),
            }
            let [.., op, _reserved, len_hi, len_lo] = header;
            let mut payload = vec![0u8; usize::from(u16::from_be_bytes([len_hi, len_lo]))];
            stream.read_exact(&mut payload)?;

            let reply = if header.starts_with(&MAGIC) {
                self.device().handle(op, &payload)
            } else {
                // Header format error.
                Reply::refused(103)
            };
            if reply.lost {
                return stream.shutdown(std::net::Shutdown::Both);
            }
            write_reply(&mut stream, &reply)?;
        }
    }

    fn device(&self) -> MutexGuard<'_, Device> {
        // A panic in one connection's thread must not take the device away from the others.
        self.device.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn write_reply(stream: &mut impl Write, reply: &Reply) -> io::Result<()> {
    let payload: &[u8] = if reply.code == RESPONSE_CODE_OK {
        &reply.payload
    } else {
        &[]
    };
    let len = u16::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "response exceeds u16::MAX"))?;
    let mut frame = Vec::with_capacity(crate::wire::RESPONSE_HEADER_LEN + payload.len());
    frame.extend_from_slice(&REPORTED_PROTOCOL);
    frame.extend_from_slice(&REPORTED_SOFTWARE);
    frame.extend_from_slice(&reply.code.to_be_bytes());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(&[0, 0]);
    frame.extend_from_slice(payload);
    stream.write_all(&frame)?;
    stream.flush()
}
