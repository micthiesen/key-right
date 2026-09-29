use crate::network_scan::MAX_SCAN_RESULTS;
use rs_matter::dm::clusters::net_comm::{
    NetworkCommissioningStatusEnum, NetworkScanInfo, ScanNetworksResponseBuilder, WiFiBandEnum,
    WiFiSecurityBitmap,
};
use rs_matter::dm::{CmdDetails, InvokeReply, InvokeReplyInstance, Reply};
use rs_matter::error::{Error, ErrorCode};
use rs_matter::im::encoding::{InvRespTag, IM_REVISION_TAG};
use rs_matter::im::IM_REVISION;
use rs_matter::tlv::{TLVTag, TLVWrite, TLVWriteParent};
use rs_matter::transport::exchange::MAX_EXCHANGE_TX_BUF_SIZE;
use rs_matter::utils::storage::WriteBuf;

/// Match InvokeResponder's envelope and the production ScanNetworks builder.
/// Include the largest optional CommandRef as well as 32-byte SSIDs.
fn encode_response(buffer: &mut [u8], count: usize) -> Result<usize, Error> {
    let mut writer = WriteBuf::new(buffer);
    writer.start_struct(&TLVTag::Anonymous)?;
    writer.bool(&TLVTag::Context(InvRespTag::SupressResponse as u8), false)?;
    writer.start_array(&TLVTag::Context(InvRespTag::InvokeResponses as u8))?;

    let command = CmdDetails::new(0, 0x31, 0, 1, false, Some(u16::MAX));
    let mut reply = InvokeReplyInstance::new(&command, &mut writer).with_command(1)?;
    {
        let tag = Reply::tag(&reply);
        let mut results =
            ScanNetworksResponseBuilder::new(TLVWriteParent::new((), reply.writer()), tag)?
                .networking_status(NetworkCommissioningStatusEnum::Success)?
                .debug_text(None)?
                .wi_fi_scan_results()?
                .some()?;
        for index in 0..count {
            let mut ssid = [b'x'; 32];
            ssid[0] = b'A' + (index % 26) as u8;
            let bssid = [0x02, 0, 0, 0, 0, index as u8];
            results = NetworkScanInfo::Wifi {
                security: WiFiSecurityBitmap::WPA_2_PERSONAL,
                ssid: &ssid,
                bssid: &bssid,
                channel: 13,
                band: WiFiBandEnum::V2G4,
                rssi: i8::MIN,
            }
            .wifi_read_into(results.push()?)?;
        }
        results.end()?.thread_scan_results()?.none().end()?;
    }
    reply.complete()?;
    writer.end_container()?;
    writer.u8(&TLVTag::Context(IM_REVISION_TAG), IM_REVISION)?;
    writer.end_container()?;
    Ok(writer.as_slice().len())
}

#[test]
fn bounded_scan_response_fits_the_real_exchange_buffer() {
    let mut buffer = [0; MAX_EXCHANGE_TX_BUF_SIZE];
    let size = encode_response(&mut buffer, MAX_SCAN_RESULTS).unwrap();
    assert_eq!(MAX_EXCHANGE_TX_BUF_SIZE, 1178);
    assert!(size < MAX_EXCHANGE_TX_BUF_SIZE);
    println!("{MAX_SCAN_RESULTS} maximum-length results: {size}/{MAX_EXCHANGE_TX_BUF_SIZE} bytes");
}

#[test]
fn unbounded_dense_scan_exhausts_the_real_exchange_buffer() {
    let mut buffer = [0; MAX_EXCHANGE_TX_BUF_SIZE];
    assert_eq!(
        encode_response(&mut buffer, 35).unwrap_err().code(),
        ErrorCode::NoSpace
    );
}
