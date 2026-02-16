use crate::opend::{
    OpenDClient, OpenDConfig, OpenDTradeHeader, PROTO_GET_GLOBAL_STATE, PROTO_INIT_CONNECT,
    PROTO_KEEP_ALIVE, PROTO_QOT_GET_BASIC_QOT, PROTO_TRD_GET_ACC_LIST, PROTO_TRD_GET_FUNDS,
    PROTO_TRD_GET_ORDER_LIST, PROTO_TRD_GET_POSITION_LIST,
};
use crate::protocol::{pack_message, unpack_header, HEAD_LEN};
use prost::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn mock_opend_roundtrip_core_apis() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        run_mock_server(listener).await;
    });

    let cfg = OpenDConfig {
        host: "127.0.0.1".to_string(),
        port: addr.port(),
        use_tls: false,
    };

    let (client, info) = OpenDClient::connect(cfg).await.unwrap();
    assert_eq!(info.conn_id, 42);
    assert!(info.keep_alive_interval_sec >= 1);

    let gs = client.get_global_state().await.unwrap();
    assert!(gs.qot_logined);
    assert!(gs.trd_logined);

    let accs = client.trd_get_acc_list().await.unwrap();
    assert_eq!(accs.len(), 1);
    let header = OpenDTradeHeader {
        trd_env: accs[0].trd_env,
        acc_id: accs[0].acc_id,
        trd_market: accs[0].trd_market_auth_list[0],
    };

    let quotes = client
        .qot_get_basic_qot(vec!["US.AAPL".to_string()])
        .await
        .unwrap();
    assert_eq!(quotes.len(), 1);
    assert_eq!(quotes[0].symbol, "US.AAPL");
    assert!(quotes[0].last > 0.0);

    let orders = client.trd_get_orders(header.clone()).await.unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].id, "OID-1");
    assert_eq!(orders[0].symbol, "US.AAPL");

    let positions = client.trd_get_positions(header.clone()).await.unwrap();
    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].symbol, "US.AAPL");
    assert_eq!(positions[0].qty, 12);

    let funds = client.trd_get_funds(header).await.unwrap().unwrap();
    assert!(funds.total_assets > 0.0);
    assert!(funds.cash >= 0.0);

    client.close().await;
    server.abort();
}

async fn run_mock_server(listener: TcpListener) {
    let (mut socket, _) = listener.accept().await.unwrap();

    loop {
        let mut head = [0u8; HEAD_LEN];
        if socket.read_exact(&mut head).await.is_err() {
            break;
        }
        let header = match unpack_header(&head) {
            Ok(h) => h,
            Err(_) => break,
        };
        let mut body = vec![0u8; header.body_len as usize];
        if socket.read_exact(&mut body).await.is_err() {
            break;
        }

        let rsp_body = mock_response_body(header.proto_id);
        let frame = pack_message(header.proto_id, header.serial_no, &rsp_body);
        if socket.write_all(&frame).await.is_err() {
            break;
        }
    }
}

fn mock_response_body(proto_id: u32) -> Vec<u8> {
    let ok = crate::pb::common::RetType::Succeed as i32;

    match proto_id {
        PROTO_INIT_CONNECT => crate::pb::init_connect::Response {
            ret_type: ok,
            ret_msg: Some("ok".to_string()),
            err_code: None,
            s2c: Some(crate::pb::init_connect::S2c {
                server_ver: 999,
                login_user_id: 1,
                conn_id: 42,
                conn_aes_key: "0123456789abcdef".to_string(),
                // Large interval prevents periodic test churn; first tick may still happen immediately.
                keep_alive_interval: 3600,
                aes_cb_civ: Some("0123456789abcdef".to_string()),
                user_attribution: None,
            }),
        }
        .encode_to_vec(),

        PROTO_KEEP_ALIVE => crate::pb::keep_alive::Response {
            ret_type: ok,
            ret_msg: Some("ok".to_string()),
            err_code: None,
            s2c: Some(crate::pb::keep_alive::S2c {
                time: chrono::Utc::now().timestamp(),
            }),
        }
        .encode_to_vec(),

        PROTO_GET_GLOBAL_STATE => crate::pb::get_global_state::Response {
            ret_type: ok,
            ret_msg: Some("ok".to_string()),
            err_code: None,
            s2c: Some(crate::pb::get_global_state::S2c {
                market_hk: crate::pb::qot_common::QotMarketState::Closed as i32,
                market_us: crate::pb::qot_common::QotMarketState::Morning as i32,
                market_sh: crate::pb::qot_common::QotMarketState::Closed as i32,
                market_sz: crate::pb::qot_common::QotMarketState::Closed as i32,
                market_hk_future: crate::pb::qot_common::QotMarketState::Closed as i32,
                market_us_future: Some(crate::pb::qot_common::QotMarketState::Closed as i32),
                market_sg_future: Some(crate::pb::qot_common::QotMarketState::Closed as i32),
                market_jp_future: Some(crate::pb::qot_common::QotMarketState::Closed as i32),
                qot_logined: true,
                trd_logined: true,
                server_ver: 999,
                server_build_no: 1,
                time: chrono::Utc::now().timestamp(),
                local_time: Some(chrono::Utc::now().timestamp_millis() as f64 / 1000.0),
                program_status: Some(crate::pb::common::ProgramStatus {
                    r#type: crate::pb::common::ProgramStatusType::Ready as i32,
                    str_ext_desc: None,
                }),
                qot_svr_ip_addr: Some("127.0.0.1".to_string()),
                trd_svr_ip_addr: Some("127.0.0.1".to_string()),
                conn_id: Some(42),
            }),
        }
        .encode_to_vec(),

        PROTO_QOT_GET_BASIC_QOT => crate::pb::qot_get_basic_qot::Response {
            ret_type: ok,
            ret_msg: Some("ok".to_string()),
            err_code: None,
            s2c: Some(crate::pb::qot_get_basic_qot::S2c {
                basic_qot_list: vec![crate::pb::qot_common::BasicQot {
                    security: crate::pb::qot_common::Security {
                        market: crate::pb::qot_common::QotMarket::UsSecurity as i32,
                        code: "AAPL".to_string(),
                    },
                    is_suspended: false,
                    list_time: "2020-01-01".to_string(),
                    price_spread: 0.01,
                    update_time: "2026-01-02 10:00:00".to_string(),
                    high_price: 124.0,
                    open_price: 122.0,
                    low_price: 121.0,
                    cur_price: 123.45,
                    last_close_price: 122.50,
                    volume: 1000,
                    turnover: 123_450.0,
                    turnover_rate: 0.0,
                    amplitude: 0.0,
                    update_timestamp: Some(1_770_000_000.0),
                    name: Some("Apple".to_string()),
                    ..Default::default()
                }],
            }),
        }
        .encode_to_vec(),

        PROTO_TRD_GET_ACC_LIST => crate::pb::trd_get_acc_list::Response {
            ret_type: ok,
            ret_msg: Some("ok".to_string()),
            err_code: None,
            s2c: Some(crate::pb::trd_get_acc_list::S2c {
                acc_list: vec![crate::pb::trd_common::TrdAcc {
                    trd_env: crate::pb::trd_common::TrdEnv::Simulate as i32,
                    acc_id: 9001,
                    trd_market_auth_list: vec![crate::pb::trd_common::TrdMarket::Us as i32],
                    ..Default::default()
                }],
            }),
        }
        .encode_to_vec(),

        PROTO_TRD_GET_ORDER_LIST => crate::pb::trd_get_order_list::Response {
            ret_type: ok,
            ret_msg: Some("ok".to_string()),
            err_code: None,
            s2c: Some(crate::pb::trd_get_order_list::S2c {
                header: crate::pb::trd_common::TrdHeader {
                    trd_env: crate::pb::trd_common::TrdEnv::Simulate as i32,
                    acc_id: 9001,
                    trd_market: crate::pb::trd_common::TrdMarket::Us as i32,
                },
                order_list: vec![crate::pb::trd_common::Order {
                    trd_side: crate::pb::trd_common::TrdSide::Buy as i32,
                    order_type: crate::pb::trd_common::OrderType::Market as i32,
                    order_status: crate::pb::trd_common::OrderStatus::Submitted as i32,
                    order_id: 101,
                    order_id_ex: "OID-1".to_string(),
                    code: "AAPL".to_string(),
                    name: "Apple".to_string(),
                    qty: 10.0,
                    price: Some(123.40),
                    create_time: "2026-01-02 10:00:00".to_string(),
                    update_time: "2026-01-02 10:00:01".to_string(),
                    fill_qty: Some(5.0),
                    fill_avg_price: Some(123.45),
                    sec_market: Some(crate::pb::trd_common::TrdSecMarket::Us as i32),
                    trd_market: Some(crate::pb::trd_common::TrdMarket::Us as i32),
                    remark: Some("cid-1".to_string()),
                    ..Default::default()
                }],
            }),
        }
        .encode_to_vec(),

        PROTO_TRD_GET_POSITION_LIST => crate::pb::trd_get_position_list::Response {
            ret_type: ok,
            ret_msg: Some("ok".to_string()),
            err_code: None,
            s2c: Some(crate::pb::trd_get_position_list::S2c {
                header: crate::pb::trd_common::TrdHeader {
                    trd_env: crate::pb::trd_common::TrdEnv::Simulate as i32,
                    acc_id: 9001,
                    trd_market: crate::pb::trd_common::TrdMarket::Us as i32,
                },
                position_list: vec![crate::pb::trd_common::Position {
                    position_id: 1,
                    position_side: crate::pb::trd_common::PositionSide::Long as i32,
                    code: "AAPL".to_string(),
                    name: "Apple".to_string(),
                    qty: 12.0,
                    can_sell_qty: 12.0,
                    price: 123.4,
                    val: 1480.8,
                    pl_val: 40.0,
                    sec_market: Some(crate::pb::trd_common::TrdSecMarket::Us as i32),
                    trd_market: Some(crate::pb::trd_common::TrdMarket::Us as i32),
                    average_cost_price: Some(120.0),
                    ..Default::default()
                }],
            }),
        }
        .encode_to_vec(),

        PROTO_TRD_GET_FUNDS => crate::pb::trd_get_funds::Response {
            ret_type: ok,
            ret_msg: Some("ok".to_string()),
            err_code: None,
            s2c: Some(crate::pb::trd_get_funds::S2c {
                header: crate::pb::trd_common::TrdHeader {
                    trd_env: crate::pb::trd_common::TrdEnv::Simulate as i32,
                    acc_id: 9001,
                    trd_market: crate::pb::trd_common::TrdMarket::Us as i32,
                },
                funds: Some(crate::pb::trd_common::Funds {
                    power: 10_000.0,
                    total_assets: 12_345.6,
                    cash: 9_876.5,
                    market_val: 2_469.1,
                    frozen_cash: 0.0,
                    debt_cash: 0.0,
                    avl_withdrawal_cash: 9_000.0,
                    ..Default::default()
                }),
            }),
        }
        .encode_to_vec(),

        _ => Vec::new(),
    }
}
