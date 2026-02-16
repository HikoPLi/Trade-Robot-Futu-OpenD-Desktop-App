use crate::{
    parse_opend_addr,
    protocol::{pack_message, unpack_header, HEAD_LEN, PROTO_FMT_PROTOBUF},
    ConnectorError,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use prost::Message;
use sha1::Digest;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc,
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{broadcast, oneshot, Mutex},
};
use trader_shared::{Candle, Fill, Order, OrderSide, OrderStatus, OrderType, Position, Quote};

pub const PROTO_INIT_CONNECT: u32 = 1001;
pub const PROTO_GET_GLOBAL_STATE: u32 = 1002;
pub const PROTO_KEEP_ALIVE: u32 = 1004;

pub const PROTO_TRD_GET_ACC_LIST: u32 = 2001;
pub const PROTO_TRD_UNLOCK_TRADE: u32 = 2005;
pub const PROTO_TRD_SUB_ACC_PUSH: u32 = 2008;
pub const PROTO_TRD_GET_FUNDS: u32 = 2101;
pub const PROTO_TRD_GET_POSITION_LIST: u32 = 2102;
pub const PROTO_TRD_GET_ORDER_LIST: u32 = 2201;
pub const PROTO_TRD_PLACE_ORDER: u32 = 2202;
pub const PROTO_TRD_MODIFY_ORDER: u32 = 2205;
pub const PROTO_TRD_UPDATE_ORDER: u32 = 2208;
pub const PROTO_TRD_UPDATE_ORDER_FILL: u32 = 2218;

pub const PROTO_QOT_SUB: u32 = 3001;
pub const PROTO_QOT_REG_QOT_PUSH: u32 = 3002;
pub const PROTO_QOT_GET_BASIC_QOT: u32 = 3004;
pub const PROTO_QOT_UPDATE_BASIC_QOT: u32 = 3005;
pub const PROTO_QOT_GET_KL: u32 = 3006;
pub const PROTO_QOT_UPDATE_KL: u32 = 3007;

#[derive(Debug, Clone)]
pub struct OpenDConfig {
    pub host: String,
    pub port: u16,
    pub use_tls: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpenDEvent {
    Quote(Quote),
    Candle(Candle),
    OrderUpdated(Order),
    Fill(Fill),
    Position(Position),
    Info { message: String },
}

#[derive(Debug, Clone)]
pub struct OpenDConnectionInfo {
    pub conn_id: u64,
    pub keep_alive_interval_sec: u32,
    pub server_ver: i32,
    pub login_user_id: u64,
}

#[derive(Debug, Clone)]
pub struct OpenDTradeAccount {
    pub trd_env: i32,
    pub acc_id: u64,
    pub trd_market_auth_list: Vec<i32>,
}

#[derive(Debug, Clone)]
pub struct OpenDTradeHeader {
    pub trd_env: i32,
    pub acc_id: u64,
    pub trd_market: i32,
}

#[derive(Debug, Clone)]
pub struct OpenDOrderRef {
    pub order_id: u64,
    pub order_id_ex: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OpenDPlaceOrderRequest {
    pub symbol: String,
    pub side: OrderSide,
    pub qty: u32,
    pub order_type: OrderType,
    pub limit_price: Option<f64>,
    pub client_order_id: String,
}

#[derive(Debug, Clone)]
pub struct RawMessage {
    pub proto_id: u32,
    pub serial_no: u32,
    pub body: Vec<u8>,
}

type PendingSender = oneshot::Sender<Result<RawMessage, ConnectorError>>;

struct Inner {
    writer: Mutex<tokio::net::tcp::OwnedWriteHalf>,
    pending: Mutex<HashMap<u32, PendingSender>>,
    event_tx: broadcast::Sender<OpenDEvent>,

    conn_id: AtomicU64,
    keep_alive_interval_sec: AtomicU32,
    next_serial: AtomicU32,
    next_packet_serial: AtomicU32,
}

#[derive(Clone)]
pub struct OpenDClient {
    inner: Arc<Inner>,
}

impl OpenDClient {
    pub async fn connect(cfg: OpenDConfig) -> Result<(Self, OpenDConnectionInfo), ConnectorError> {
        if cfg.use_tls {
            // OpenD uses a proprietary binary protocol; secure remote usage is typically via SSH/VPN.
            return Err(ConnectorError::Protocol(
                "TLS is not supported by the OpenD protocol in MVP".to_string(),
            ));
        }

        let addr = parse_opend_addr(&cfg.host, cfg.port)
            .map_err(|e| ConnectorError::ConnectionFailed(e.to_string()))?;
        let stream = tokio::time::timeout(std::time::Duration::from_secs(5), TcpStream::connect(addr))
            .await
            .map_err(|_| ConnectorError::Timeout("connect timeout".to_string()))?
            .map_err(|e| ConnectorError::ConnectionFailed(e.to_string()))?;
        stream
            .set_nodelay(true)
            .map_err(|e| ConnectorError::ConnectionFailed(e.to_string()))?;

        let (reader, writer) = stream.into_split();
        let (event_tx, _) = broadcast::channel(2048);

        let inner = Arc::new(Inner {
            writer: Mutex::new(writer),
            pending: Mutex::new(HashMap::new()),
            event_tx,
            conn_id: AtomicU64::new(0),
            keep_alive_interval_sec: AtomicU32::new(10),
            next_serial: AtomicU32::new(1),
            next_packet_serial: AtomicU32::new(1),
        });

        let client = Self { inner: inner.clone() };
        tokio::spawn(read_loop(reader, inner.clone()));

        // InitConnect handshake.
        let info = client.init_connect().await?;

        // Start keepalive loop.
        let ka = info.keep_alive_interval_sec;
        let client2 = client.clone();
        tokio::spawn(async move {
            client2.keep_alive_loop(ka).await;
        });

        Ok((client, info))
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<OpenDEvent> {
        self.inner.event_tx.subscribe()
    }

    pub async fn test_connection(cfg: OpenDConfig) -> Result<OpenDConnectionInfo, ConnectorError> {
        let (c, info) = Self::connect(cfg).await?;
        c.close().await;
        Ok(info)
    }

    /// Best-effort graceful shutdown of the underlying TCP connection.
    pub async fn close(&self) {
        let mut w = self.inner.writer.lock().await;
        let _ = w.shutdown().await;
    }

    async fn request_raw(
        &self,
        proto_id: u32,
        body: Vec<u8>,
        timeout: std::time::Duration,
    ) -> Result<RawMessage, ConnectorError> {
        let serial_no = self.inner.next_serial.fetch_add(1, Ordering::Relaxed);

        let msg = pack_message(proto_id, serial_no, &body);
        let (tx, rx) = oneshot::channel::<Result<RawMessage, ConnectorError>>();
        {
            let mut pending = self.inner.pending.lock().await;
            pending.insert(serial_no, tx);
        }

        {
            let mut w = self.inner.writer.lock().await;
            w.write_all(&msg)
                .await
                .map_err(|e| ConnectorError::ConnectionFailed(e.to_string()))?;
        }

        let res = tokio::time::timeout(timeout, rx)
            .await
            .map_err(|_| ConnectorError::Timeout(format!("proto {proto_id} timeout")))?;

        res.map_err(|_| ConnectorError::Protocol("response channel closed".to_string()))?
    }

    async fn init_connect(&self) -> Result<OpenDConnectionInfo, ConnectorError> {
        let c2s = crate::pb::init_connect::C2s {
            client_ver: 300,
            client_id: "TradeRobot".to_string(),
            recv_notify: Some(true),
            packet_enc_algo: Some(crate::pb::common::PacketEncAlgo::None as i32),
            push_proto_fmt: Some(crate::pb::common::ProtoFmt::Protobuf as i32),
            programming_language: Some("Rust".to_string()),
        };
        let req = crate::pb::init_connect::Request { c2s };

        let raw = self
            .request_raw(PROTO_INIT_CONNECT, req.encode_to_vec(), std::time::Duration::from_secs(8))
            .await?;
        let rsp = crate::pb::init_connect::Response::decode(raw.body.as_slice())
            .map_err(|e| ConnectorError::Protocol(format!("decode init_connect: {e}")))?;

        if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
            return Err(ConnectorError::RemoteError {
                proto_id: PROTO_INIT_CONNECT,
                ret_type: rsp.ret_type,
                ret_msg: rsp.ret_msg.unwrap_or_default(),
            });
        }

        let s2c = rsp.s2c.ok_or_else(|| ConnectorError::Protocol("init_connect missing s2c".to_string()))?;

        let conn_id = s2c.conn_id;
        let keep_alive_interval = s2c.keep_alive_interval.max(1) as u32;

        self.inner.conn_id.store(conn_id, Ordering::Relaxed);
        self.inner
            .keep_alive_interval_sec
            .store(keep_alive_interval, Ordering::Relaxed);

        Ok(OpenDConnectionInfo {
            conn_id,
            keep_alive_interval_sec: keep_alive_interval,
            server_ver: s2c.server_ver,
            login_user_id: s2c.login_user_id,
        })
    }

    async fn keep_alive_loop(&self, interval_sec: u32) {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_sec as u64));
        loop {
            ticker.tick().await;
            let req = crate::pb::keep_alive::Request { c2s: crate::pb::keep_alive::C2s { time: Utc::now().timestamp() } };
            let _ = self
                .request_raw(PROTO_KEEP_ALIVE, req.encode_to_vec(), std::time::Duration::from_secs(5))
                .await;
        }
    }

    pub async fn get_global_state(&self) -> Result<crate::pb::get_global_state::S2c, ConnectorError> {
        let req = crate::pb::get_global_state::Request { c2s: crate::pb::get_global_state::C2s { user_id: 0 } };
        let raw = self
            .request_raw(PROTO_GET_GLOBAL_STATE, req.encode_to_vec(), std::time::Duration::from_secs(8))
            .await?;
        let rsp = crate::pb::get_global_state::Response::decode(raw.body.as_slice())
            .map_err(|e| ConnectorError::Protocol(format!("decode get_global_state: {e}")))?;

        if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
            return Err(ConnectorError::RemoteError {
                proto_id: PROTO_GET_GLOBAL_STATE,
                ret_type: rsp.ret_type,
                ret_msg: rsp.ret_msg.unwrap_or_default(),
            });
        }
        rsp.s2c.ok_or_else(|| ConnectorError::Protocol("missing s2c".to_string()))
    }

    pub async fn qot_subscribe_basic(&self, symbols: Vec<String>) -> Result<(), ConnectorError> {
        let securities: Vec<crate::pb::qot_common::Security> = symbols
            .iter()
            .map(|s| to_qot_security(s))
            .collect::<Result<Vec<_>, _>>()?;

        let c2s = crate::pb::qot_sub::C2s {
            security_list: securities,
            sub_type_list: vec![crate::pb::qot_common::SubType::Basic as i32],
            is_sub_or_un_sub: true,
            is_reg_or_un_reg_push: Some(true),
            reg_push_rehab_type_list: vec![],
            is_first_push: Some(true),
            is_unsub_all: None,
            is_sub_order_book_detail: None,
            extended_time: None,
            session: None,
        };
        let req = crate::pb::qot_sub::Request { c2s };

        let raw = self
            .request_raw(PROTO_QOT_SUB, req.encode_to_vec(), std::time::Duration::from_secs(10))
            .await?;
        let rsp = crate::pb::qot_sub::Response::decode(raw.body.as_slice())
            .map_err(|e| ConnectorError::Protocol(format!("decode qot_sub: {e}")))?;
        if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
            return Err(ConnectorError::RemoteError {
                proto_id: PROTO_QOT_SUB,
                ret_type: rsp.ret_type,
                ret_msg: rsp.ret_msg.unwrap_or_default(),
            });
        }
        Ok(())
    }

    pub async fn qot_get_basic_qot(&self, symbols: Vec<String>) -> Result<Vec<Quote>, ConnectorError> {
        let securities: Vec<crate::pb::qot_common::Security> = symbols
            .iter()
            .map(|s| to_qot_security(s))
            .collect::<Result<Vec<_>, _>>()?;

        let req = crate::pb::qot_get_basic_qot::Request { c2s: crate::pb::qot_get_basic_qot::C2s { security_list: securities } };
        let raw = self
            .request_raw(PROTO_QOT_GET_BASIC_QOT, req.encode_to_vec(), std::time::Duration::from_secs(10))
            .await?;
        let rsp = crate::pb::qot_get_basic_qot::Response::decode(raw.body.as_slice())
            .map_err(|e| ConnectorError::Protocol(format!("decode qot_get_basic_qot: {e}")))?;
        if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
            return Err(ConnectorError::RemoteError {
                proto_id: PROTO_QOT_GET_BASIC_QOT,
                ret_type: rsp.ret_type,
                ret_msg: rsp.ret_msg.unwrap_or_default(),
            });
        }
        let mut out = Vec::new();
        if let Some(s2c) = rsp.s2c {
            for bq in s2c.basic_qot_list {
                if let Ok(q) = basic_qot_to_quote(&bq) {
                    out.push(q);
                }
            }
        }
        Ok(out)
    }

    pub async fn qot_get_kl(
        &self,
        symbol: String,
        interval_sec: u32,
        limit: u32,
    ) -> Result<Vec<Candle>, ConnectorError> {
        let (rehab_type, kl_type) = match interval_sec {
            60 => (
                crate::pb::qot_common::RehabType::None as i32,
                crate::pb::qot_common::KlType::KlType1min as i32,
            ),
            300 => (
                crate::pb::qot_common::RehabType::None as i32,
                crate::pb::qot_common::KlType::KlType5min as i32,
            ),
            _ => {
                return Err(ConnectorError::Protocol(format!(
                    "unsupported interval_sec for OpenD KL: {interval_sec}"
                )))
            }
        };

        let sec = to_qot_security(&symbol)?;
        let req = crate::pb::qot_get_kl::Request { c2s: crate::pb::qot_get_kl::C2s { rehab_type, kl_type, security: sec, req_num: limit as i32 } };
        let raw = self
            .request_raw(PROTO_QOT_GET_KL, req.encode_to_vec(), std::time::Duration::from_secs(10))
            .await?;
        let rsp = crate::pb::qot_get_kl::Response::decode(raw.body.as_slice())
            .map_err(|e| ConnectorError::Protocol(format!("decode qot_get_kl: {e}")))?;
        if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
            return Err(ConnectorError::RemoteError {
                proto_id: PROTO_QOT_GET_KL,
                ret_type: rsp.ret_type,
                ret_msg: rsp.ret_msg.unwrap_or_default(),
            });
        }

        let s2c = rsp.s2c.ok_or_else(|| ConnectorError::Protocol("missing s2c".to_string()))?;
        let mut out = Vec::new();
        for kl in s2c.kl_list {
            if kl.is_blank {
                continue;
            }
            if let Ok(c) = kline_to_candle(&symbol, interval_sec, &kl) {
                out.push(c);
            }
        }
        Ok(out)
    }

    pub async fn trd_get_acc_list(&self) -> Result<Vec<OpenDTradeAccount>, ConnectorError> {
        let req = crate::pb::trd_get_acc_list::Request { c2s: crate::pb::trd_get_acc_list::C2s { user_id: 0, trd_category: None, need_general_sec_account: None } };
        let raw = self
            .request_raw(PROTO_TRD_GET_ACC_LIST, req.encode_to_vec(), std::time::Duration::from_secs(10))
            .await?;
        let rsp = crate::pb::trd_get_acc_list::Response::decode(raw.body.as_slice())
            .map_err(|e| ConnectorError::Protocol(format!("decode trd_get_acc_list: {e}")))?;
        if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
            return Err(ConnectorError::RemoteError {
                proto_id: PROTO_TRD_GET_ACC_LIST,
                ret_type: rsp.ret_type,
                ret_msg: rsp.ret_msg.unwrap_or_default(),
            });
        }

        let mut out = Vec::new();
        if let Some(s2c) = rsp.s2c {
            for acc in s2c.acc_list {
                out.push(OpenDTradeAccount {
                    trd_env: acc.trd_env,
                    acc_id: acc.acc_id,
                    trd_market_auth_list: acc.trd_market_auth_list,
                });
            }
        }
        Ok(out)
    }

    pub async fn trd_unlock_trade(&self, pwd_md5_hex_lower: String) -> Result<(), ConnectorError> {
        let req = crate::pb::trd_unlock_trade::Request { c2s: crate::pb::trd_unlock_trade::C2s { unlock: true, pwd_md5: Some(pwd_md5_hex_lower), security_firm: None } };
        let raw = self
            .request_raw(PROTO_TRD_UNLOCK_TRADE, req.encode_to_vec(), std::time::Duration::from_secs(10))
            .await?;
        let rsp = crate::pb::trd_unlock_trade::Response::decode(raw.body.as_slice())
            .map_err(|e| ConnectorError::Protocol(format!("decode trd_unlock_trade: {e}")))?;
        if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
            return Err(ConnectorError::RemoteError {
                proto_id: PROTO_TRD_UNLOCK_TRADE,
                ret_type: rsp.ret_type,
                ret_msg: rsp.ret_msg.unwrap_or_default(),
            });
        }
        Ok(())
    }

    pub async fn trd_sub_acc_push(&self, acc_ids: Vec<u64>) -> Result<(), ConnectorError> {
        let req = crate::pb::trd_sub_acc_push::Request { c2s: crate::pb::trd_sub_acc_push::C2s { acc_id_list: acc_ids } };
        let raw = self
            .request_raw(PROTO_TRD_SUB_ACC_PUSH, req.encode_to_vec(), std::time::Duration::from_secs(10))
            .await?;
        let rsp = crate::pb::trd_sub_acc_push::Response::decode(raw.body.as_slice())
            .map_err(|e| ConnectorError::Protocol(format!("decode trd_sub_acc_push: {e}")))?;
        if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
            return Err(ConnectorError::RemoteError {
                proto_id: PROTO_TRD_SUB_ACC_PUSH,
                ret_type: rsp.ret_type,
                ret_msg: rsp.ret_msg.unwrap_or_default(),
            });
        }
        Ok(())
    }

    pub async fn trd_place_order(
        &self,
        header: OpenDTradeHeader,
        req: OpenDPlaceOrderRequest,
    ) -> Result<OpenDOrderRef, ConnectorError> {
        let (trd_market, sec_market, code) = to_trade_routing(&req.symbol)?;
        if header.trd_market != trd_market {
            return Err(ConnectorError::Protocol(format!(
                "trade header market mismatch: header.trd_market={} symbol_market={}",
                header.trd_market, trd_market
            )));
        }

        let trd_side = match req.side {
            OrderSide::Buy => crate::pb::trd_common::TrdSide::Buy as i32,
            OrderSide::Sell => crate::pb::trd_common::TrdSide::Sell as i32,
        };
        let ot = match req.order_type {
            OrderType::Market => crate::pb::trd_common::OrderType::Market as i32,
            OrderType::Limit => crate::pb::trd_common::OrderType::Normal as i32,
        };

        let conn_id = self.inner.conn_id.load(Ordering::Relaxed);
        let packet_serial = self.inner.next_packet_serial.fetch_add(1, Ordering::Relaxed);

        let req = crate::pb::trd_place_order::Request {
            c2s: crate::pb::trd_place_order::C2s {
                packet_id: crate::pb::common::PacketId {
                    conn_id,
                    serial_no: packet_serial,
                },
                header: crate::pb::trd_common::TrdHeader {
                    trd_env: header.trd_env,
                    acc_id: header.acc_id,
                    trd_market,
                },
                trd_side,
                order_type: ot,
                code,
                qty: req.qty as f64,
                price: req.limit_price,
                adjust_price: Some(true),
                adjust_side_and_limit: None,
                sec_market: Some(sec_market),
                remark: Some(truncate_remark(&req.client_order_id)),
                time_in_force: None,
                fill_outside_rth: None,
                aux_price: None,
                trail_type: None,
                trail_value: None,
                trail_spread: None,
                session: None,
            },
        };

        let raw = self
            .request_raw(PROTO_TRD_PLACE_ORDER, req.encode_to_vec(), std::time::Duration::from_secs(12))
            .await?;
        let rsp = crate::pb::trd_place_order::Response::decode(raw.body.as_slice())
            .map_err(|e| ConnectorError::Protocol(format!("decode trd_place_order: {e}")))?;
        if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
            return Err(ConnectorError::RemoteError {
                proto_id: PROTO_TRD_PLACE_ORDER,
                ret_type: rsp.ret_type,
                ret_msg: rsp.ret_msg.unwrap_or_default(),
            });
        }
        let s2c = rsp.s2c.ok_or_else(|| ConnectorError::Protocol("missing s2c".to_string()))?;
        Ok(OpenDOrderRef {
            order_id: s2c.order_id.unwrap_or(0),
            order_id_ex: s2c.order_id_ex,
        })
    }

    pub async fn trd_cancel_order(&self, header: OpenDTradeHeader, order_id_ex: String) -> Result<(), ConnectorError> {
        let trd_market = header.trd_market;

        let conn_id = self.inner.conn_id.load(Ordering::Relaxed);
        let packet_serial = self.inner.next_packet_serial.fetch_add(1, Ordering::Relaxed);

        let req = crate::pb::trd_modify_order::Request {
            c2s: crate::pb::trd_modify_order::C2s {
                packet_id: crate::pb::common::PacketId {
                    conn_id,
                    serial_no: packet_serial,
                },
                header: crate::pb::trd_common::TrdHeader {
                    trd_env: header.trd_env,
                    acc_id: header.acc_id,
                    trd_market,
                },
                order_id: 0,
                modify_order_op: crate::pb::trd_common::ModifyOrderOp::Cancel as i32,
                for_all: None,
                trd_market: None,
                qty: None,
                price: None,
                adjust_price: None,
                adjust_side_and_limit: None,
                aux_price: None,
                trail_type: None,
                trail_value: None,
                trail_spread: None,
                order_id_ex: Some(order_id_ex),
            },
        };

        let raw = self
            .request_raw(PROTO_TRD_MODIFY_ORDER, req.encode_to_vec(), std::time::Duration::from_secs(12))
            .await?;
        let rsp = crate::pb::trd_modify_order::Response::decode(raw.body.as_slice())
            .map_err(|e| ConnectorError::Protocol(format!("decode trd_modify_order: {e}")))?;
        if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
            return Err(ConnectorError::RemoteError {
                proto_id: PROTO_TRD_MODIFY_ORDER,
                ret_type: rsp.ret_type,
                ret_msg: rsp.ret_msg.unwrap_or_default(),
            });
        }
        Ok(())
    }

    pub async fn trd_get_order_list(&self, header: OpenDTradeHeader) -> Result<Vec<crate::pb::trd_common::Order>, ConnectorError> {
        let req = crate::pb::trd_get_order_list::Request {
            c2s: crate::pb::trd_get_order_list::C2s {
                header: crate::pb::trd_common::TrdHeader {
                    trd_env: header.trd_env,
                    acc_id: header.acc_id,
                    trd_market: header.trd_market,
                },
                filter_conditions: None,
                filter_status_list: vec![],
                refresh_cache: Some(true),
            },
        };

        let raw = self
            .request_raw(PROTO_TRD_GET_ORDER_LIST, req.encode_to_vec(), std::time::Duration::from_secs(10))
            .await?;
        let rsp = crate::pb::trd_get_order_list::Response::decode(raw.body.as_slice())
            .map_err(|e| ConnectorError::Protocol(format!("decode trd_get_order_list: {e}")))?;
        if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
            return Err(ConnectorError::RemoteError {
                proto_id: PROTO_TRD_GET_ORDER_LIST,
                ret_type: rsp.ret_type,
                ret_msg: rsp.ret_msg.unwrap_or_default(),
            });
        }
        let s2c = rsp.s2c.ok_or_else(|| ConnectorError::Protocol("missing s2c".to_string()))?;
        Ok(s2c.order_list)
    }

    pub async fn trd_get_orders(&self, header: OpenDTradeHeader) -> Result<Vec<Order>, ConnectorError> {
        let list = self.trd_get_order_list(header).await?;
        let mut out = Vec::new();
        for o in list {
            let symbol = format_trd_symbol(o.trd_market, o.sec_market, Some(o.code.as_str()));
            out.push(trd_order_to_order(&symbol, &o));
        }
        Ok(out)
    }

    pub async fn trd_get_position_list(
        &self,
        header: OpenDTradeHeader,
    ) -> Result<Vec<crate::pb::trd_common::Position>, ConnectorError> {
        let req = crate::pb::trd_get_position_list::Request {
            c2s: crate::pb::trd_get_position_list::C2s {
                header: crate::pb::trd_common::TrdHeader {
                    trd_env: header.trd_env,
                    acc_id: header.acc_id,
                    trd_market: header.trd_market,
                },
                filter_conditions: None,
                filter_pl_ratio_min: None,
                filter_pl_ratio_max: None,
                refresh_cache: Some(true),
            },
        };
        let raw = self
            .request_raw(PROTO_TRD_GET_POSITION_LIST, req.encode_to_vec(), std::time::Duration::from_secs(10))
            .await?;
        let rsp = crate::pb::trd_get_position_list::Response::decode(raw.body.as_slice())
            .map_err(|e| ConnectorError::Protocol(format!("decode trd_get_position_list: {e}")))?;
        if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
            return Err(ConnectorError::RemoteError {
                proto_id: PROTO_TRD_GET_POSITION_LIST,
                ret_type: rsp.ret_type,
                ret_msg: rsp.ret_msg.unwrap_or_default(),
            });
        }
        let s2c = rsp.s2c.ok_or_else(|| ConnectorError::Protocol("missing s2c".to_string()))?;
        Ok(s2c.position_list)
    }

    pub async fn trd_get_positions(&self, header: OpenDTradeHeader) -> Result<Vec<Position>, ConnectorError> {
        let list = self.trd_get_position_list(header).await?;
        let mut out = Vec::new();
        for p in list {
            let symbol = format_trd_symbol(p.trd_market, p.sec_market, Some(p.code.as_str()));
            out.push(trd_position_to_position(&symbol, &p));
        }
        Ok(out)
    }

    pub async fn trd_get_funds(
        &self,
        header: OpenDTradeHeader,
    ) -> Result<Option<crate::pb::trd_common::Funds>, ConnectorError> {
        let req = crate::pb::trd_get_funds::Request {
            c2s: crate::pb::trd_get_funds::C2s {
                header: crate::pb::trd_common::TrdHeader {
                    trd_env: header.trd_env,
                    acc_id: header.acc_id,
                    trd_market: header.trd_market,
                },
                refresh_cache: Some(true),
                currency: None,
            },
        };
        let raw = self
            .request_raw(PROTO_TRD_GET_FUNDS, req.encode_to_vec(), std::time::Duration::from_secs(10))
            .await?;
        let rsp = crate::pb::trd_get_funds::Response::decode(raw.body.as_slice())
            .map_err(|e| ConnectorError::Protocol(format!("decode trd_get_funds: {e}")))?;
        if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
            return Err(ConnectorError::RemoteError {
                proto_id: PROTO_TRD_GET_FUNDS,
                ret_type: rsp.ret_type,
                ret_msg: rsp.ret_msg.unwrap_or_default(),
            });
        }
        Ok(rsp.s2c.and_then(|s| s.funds))
    }
}

async fn read_loop(mut reader: tokio::net::tcp::OwnedReadHalf, inner: Arc<Inner>) {
    loop {
        let mut head_buf = [0u8; HEAD_LEN];
        if let Err(e) = reader.read_exact(&mut head_buf).await {
            let _ = inner.event_tx.send(OpenDEvent::Info {
                message: format!("OpenD read failed: {e}"),
            });
            break;
        }

        let header = match unpack_header(&head_buf) {
            Ok(h) => h,
            Err(e) => {
                let _ = inner.event_tx.send(OpenDEvent::Info {
                    message: format!("OpenD invalid header: {e}"),
                });
                break;
            }
        };

        if header.proto_fmt_type != PROTO_FMT_PROTOBUF {
            let _ = inner.event_tx.send(OpenDEvent::Info {
                message: format!(
                    "OpenD unsupported proto_fmt_type {} (only protobuf supported)",
                    header.proto_fmt_type
                ),
            });
            break;
        }

        let body_len = header.body_len as usize;
        let mut body = vec![0u8; body_len];
        if let Err(e) = reader.read_exact(&mut body).await {
            let _ = inner.event_tx.send(OpenDEvent::Info {
                message: format!("OpenD body read failed: {e}"),
            });
            break;
        }

        // Verify SHA1 of plaintext body.
        let digest: [u8; 20] = sha1::Sha1::digest(body.as_slice()).into();
        if digest != header.sha1 {
            let _ = inner.event_tx.send(OpenDEvent::Info {
                message: format!("OpenD sha1 mismatch proto_id={}", header.proto_id),
            });
            continue;
        }

        if is_push_proto(header.proto_id) {
            handle_push(header.proto_id, &body, &inner.event_tx);
            continue;
        }

        let tx_opt = { inner.pending.lock().await.remove(&header.serial_no) };
        if let Some(tx) = tx_opt {
            let _ = tx.send(Ok(RawMessage {
                proto_id: header.proto_id,
                serial_no: header.serial_no,
                body,
            }));
        }
    }

    // Fail outstanding requests.
    let pending = std::mem::take(&mut *inner.pending.lock().await);
    for (_k, tx) in pending {
        let _ = tx.send(Err(ConnectorError::ConnectionFailed(
            "connection closed".to_string(),
        )));
    }
}

fn is_push_proto(proto_id: u32) -> bool {
    matches!(
        proto_id,
        PROTO_TRD_UPDATE_ORDER
            | PROTO_TRD_UPDATE_ORDER_FILL
            | PROTO_QOT_UPDATE_BASIC_QOT
            | PROTO_QOT_UPDATE_KL
    )
}

fn handle_push(proto_id: u32, body: &[u8], event_tx: &broadcast::Sender<OpenDEvent>) {
    match proto_id {
        PROTO_QOT_UPDATE_BASIC_QOT => {
            if let Ok(rsp) = crate::pb::qot_update_basic_qot::Response::decode(body) {
                if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
                    return;
                }
                if let Some(s2c) = rsp.s2c {
                    for bq in s2c.basic_qot_list {
                        if let Ok(q) = basic_qot_to_quote(&bq) {
                            let _ = event_tx.send(OpenDEvent::Quote(q));
                        }
                    }
                }
            }
        }
        PROTO_QOT_UPDATE_KL => {
            if let Ok(rsp) = crate::pb::qot_update_kl::Response::decode(body) {
                if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
                    return;
                }
                let Some(s2c) = rsp.s2c else { return };
                let symbol = format_symbol_from_security(&s2c.security);
                let interval_sec = match s2c.kl_type {
                    x if x == crate::pb::qot_common::KlType::KlType1min as i32 => 60,
                    x if x == crate::pb::qot_common::KlType::KlType5min as i32 => 300,
                    _ => 0,
                };
                if interval_sec == 0 {
                    return;
                }
                for kl in s2c.kl_list {
                    if kl.is_blank {
                        continue;
                    }
                    if let Ok(c) = kline_to_candle(&symbol, interval_sec, &kl) {
                        let _ = event_tx.send(OpenDEvent::Candle(c));
                    }
                }
            }
        }
        PROTO_TRD_UPDATE_ORDER => {
            if let Ok(rsp) = crate::pb::trd_update_order::Response::decode(body) {
                if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
                    return;
                }
                let Some(s2c) = rsp.s2c else { return };
                let o = s2c.order;
                let symbol = format_trd_symbol(o.trd_market, o.sec_market, Some(o.code.as_str()));
                let _ = event_tx.send(OpenDEvent::OrderUpdated(trd_order_to_order(&symbol, &o)));
            }
        }
        PROTO_TRD_UPDATE_ORDER_FILL => {
            if let Ok(rsp) = crate::pb::trd_update_order_fill::Response::decode(body) {
                if rsp.ret_type != crate::pb::common::RetType::Succeed as i32 {
                    return;
                }
                let Some(s2c) = rsp.s2c else { return };
                let f = s2c.order_fill;
                let symbol = format_trd_symbol(f.trd_market, f.sec_market, Some(f.code.as_str()));
                let _ = event_tx.send(OpenDEvent::Fill(trd_fill_to_fill(&symbol, &f)));
            }
        }
        _ => {}
    }
}

fn truncate_remark(s: &str) -> String {
    // remark is limited to 64 bytes. Keep it ASCII to avoid truncating mid-UTF8.
    let mut out = String::new();
    for ch in s.chars().filter(|c| c.is_ascii()) {
        out.push(ch);
        if out.len() >= 64 {
            break;
        }
    }
    out
}

fn to_qot_security(symbol: &str) -> Result<crate::pb::qot_common::Security, ConnectorError> {
    let (mkt, code) = parse_futu_symbol(symbol)?;
    Ok(crate::pb::qot_common::Security {
        market: mkt,
        code,
    })
}

fn parse_futu_symbol(symbol: &str) -> Result<(i32, String), ConnectorError> {
    let s = symbol.trim();
    if s.is_empty() {
        return Err(ConnectorError::Protocol("symbol is empty".to_string()));
    }
    if let Some((prefix, code)) = s.split_once('.') {
        let prefix = prefix.to_uppercase();
        let code = code.to_string();
        let market = match prefix.as_str() {
            "US" => crate::pb::qot_common::QotMarket::UsSecurity as i32,
            "HK" => crate::pb::qot_common::QotMarket::HkSecurity as i32,
            "SH" => crate::pb::qot_common::QotMarket::CnshSecurity as i32,
            "SZ" => crate::pb::qot_common::QotMarket::CnszSecurity as i32,
            _ => crate::pb::qot_common::QotMarket::UsSecurity as i32,
        };
        return Ok((market, code));
    }
    Ok((
        crate::pb::qot_common::QotMarket::UsSecurity as i32,
        s.to_string(),
    ))
}

fn format_symbol_from_security(sec: &crate::pb::qot_common::Security) -> String {
    let market = sec.market;
    let code = sec.code.as_str();
    let prefix = match market {
        x if x == crate::pb::qot_common::QotMarket::UsSecurity as i32 => "US",
        x if x == crate::pb::qot_common::QotMarket::HkSecurity as i32 => "HK",
        x if x == crate::pb::qot_common::QotMarket::CnshSecurity as i32 => "SH",
        x if x == crate::pb::qot_common::QotMarket::CnszSecurity as i32 => "SZ",
        _ => "UNKNOWN",
    };
    format!("{prefix}.{code}")
}

fn basic_qot_to_quote(bq: &crate::pb::qot_common::BasicQot) -> Result<Quote, ConnectorError> {
    let symbol = format_symbol_from_security(&bq.security);
    let ts = if let Some(t) = bq.update_timestamp {
        // API uses seconds timestamp (double). Convert to millis.
        let millis = (t * 1000.0) as i64;
        DateTime::<Utc>::from_timestamp_millis(millis).unwrap_or_else(Utc::now)
    } else {
        parse_opend_dt(&bq.update_time).unwrap_or_else(Utc::now)
    };

    let last = bq.cur_price;
    let vol = bq.volume;

    Ok(Quote {
        symbol,
        ts,
        bid: last,
        ask: last,
        last,
        volume: vol as f64,
    })
}

fn parse_opend_dt(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // OpenD commonly uses "YYYY-MM-DD HH:MM:SS" with optional fractional seconds.
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc));
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc));
    }

    // Best-effort: accept RFC3339 if it ever appears.
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    None
}

fn kline_to_candle(symbol: &str, interval_sec: u32, kl: &crate::pb::qot_common::KLine) -> Result<Candle, ConnectorError> {
    let ts = if let Some(t) = kl.timestamp {
        let millis = (t * 1000.0) as i64;
        DateTime::<Utc>::from_timestamp_millis(millis).unwrap_or_else(Utc::now)
    } else {
        parse_opend_dt(&kl.time).unwrap_or_else(Utc::now)
    };

    Ok(Candle {
        symbol: symbol.to_string(),
        ts,
        interval_sec,
        open: kl.open_price.unwrap_or(0.0),
        high: kl.high_price.unwrap_or(0.0),
        low: kl.low_price.unwrap_or(0.0),
        close: kl.close_price.unwrap_or(0.0),
        volume: kl.volume.unwrap_or(0) as f64,
    })
}

fn trd_order_to_order(symbol: &str, o: &crate::pb::trd_common::Order) -> Order {
    let status = match o.order_status {
        x if x == crate::pb::trd_common::OrderStatus::Unsubmitted as i32
            || x == crate::pb::trd_common::OrderStatus::WaitingSubmit as i32
            || x == crate::pb::trd_common::OrderStatus::Submitting as i32 =>
        {
            OrderStatus::PendingSubmit
        }
        x if x == crate::pb::trd_common::OrderStatus::Submitted as i32
            || x == crate::pb::trd_common::OrderStatus::FilledPart as i32
            || x == crate::pb::trd_common::OrderStatus::CancellingPart as i32
            || x == crate::pb::trd_common::OrderStatus::CancellingAll as i32 =>
        {
            OrderStatus::Submitted
        }
        x if x == crate::pb::trd_common::OrderStatus::FilledAll as i32 => OrderStatus::Filled,
        x if x == crate::pb::trd_common::OrderStatus::CancelledAll as i32
            || x == crate::pb::trd_common::OrderStatus::CancelledPart as i32
            || x == crate::pb::trd_common::OrderStatus::Deleted as i32
            || x == crate::pb::trd_common::OrderStatus::Disabled as i32 =>
        {
            OrderStatus::Cancelled
        }
        x if x == crate::pb::trd_common::OrderStatus::SubmitFailed as i32
            || x == crate::pb::trd_common::OrderStatus::Failed as i32
            || x == crate::pb::trd_common::OrderStatus::TimeOut as i32
            || x == crate::pb::trd_common::OrderStatus::Unknown as i32 =>
        {
            OrderStatus::Rejected
        }
        _ => OrderStatus::Submitted,
    };

    let order_type = if o.order_type == crate::pb::trd_common::OrderType::Market as i32 {
        OrderType::Market
    } else {
        OrderType::Limit
    };

    let side = match o.trd_side {
        x if x == crate::pb::trd_common::TrdSide::Buy as i32
            || x == crate::pb::trd_common::TrdSide::BuyBack as i32 =>
        {
            OrderSide::Buy
        }
        _ => OrderSide::Sell,
    };

    let created_at = if let Some(t) = o.create_timestamp {
        DateTime::<Utc>::from_timestamp_millis((t * 1000.0) as i64).unwrap_or_else(Utc::now)
    } else {
        parse_opend_dt(&o.create_time).unwrap_or_else(Utc::now)
    };
    let updated_at = if let Some(t) = o.update_timestamp {
        DateTime::<Utc>::from_timestamp_millis((t * 1000.0) as i64).unwrap_or_else(Utc::now)
    } else {
        parse_opend_dt(&o.update_time).unwrap_or_else(Utc::now)
    };

    Order {
        id: o.order_id_ex.clone(),
        symbol: symbol.to_string(),
        side,
        qty: o.qty.floor().max(0.0) as u32,
        order_type,
        limit_price: o.price,
        status,
        filled_qty: o.fill_qty.unwrap_or(0.0).floor().max(0.0) as u32,
        avg_fill_price: o.fill_avg_price,
        created_at,
        updated_at,
        client_order_id: o.remark.clone().unwrap_or_default(),
        last_error: o.last_err_msg.clone(),
    }
}

fn trd_fill_to_fill(symbol: &str, f: &crate::pb::trd_common::OrderFill) -> Fill {
    let side = match f.trd_side {
        x if x == crate::pb::trd_common::TrdSide::Buy as i32
            || x == crate::pb::trd_common::TrdSide::BuyBack as i32 =>
        {
            OrderSide::Buy
        }
        _ => OrderSide::Sell,
    };
    let ts = if let Some(t) = f.create_timestamp {
        DateTime::<Utc>::from_timestamp_millis((t * 1000.0) as i64).unwrap_or_else(Utc::now)
    } else {
        parse_opend_dt(&f.create_time).unwrap_or_else(Utc::now)
    };
    Fill {
        order_id: f.order_id_ex.clone().unwrap_or_else(|| f.order_id.unwrap_or(0).to_string()),
        symbol: symbol.to_string(),
        side,
        qty: f.qty.floor().max(0.0) as u32,
        price: f.price,
        fee: 0.0,
        ts,
    }
}

fn trd_position_to_position(symbol: &str, p: &crate::pb::trd_common::Position) -> Position {
    let side = p.position_side;
    let qty_abs = p.qty.floor().max(0.0) as i64;
    let qty = if side == crate::pb::trd_common::PositionSide::Short as i32 {
        -qty_abs
    } else {
        qty_abs
    };
    let avg_cost = p
        .average_cost_price
        .or(p.diluted_cost_price)
        .or(p.cost_price)
        .unwrap_or(0.0);
    // OpenD provides `pl_val` (profit/loss). For securities accounts this is typically unrealized.
    let pl = p.pl_val;
    Position {
        symbol: symbol.to_string(),
        qty,
        avg_cost,
        realized_pnl: pl,
        updated_at: Utc::now(),
    }
}

fn to_trade_routing(symbol: &str) -> Result<(i32, i32, String), ConnectorError> {
    let s = symbol.trim();
    if s.is_empty() {
        return Err(ConnectorError::Protocol("symbol is empty".to_string()));
    }

    let (prefix, code) = s.split_once('.').unwrap_or(("US", s));
    let prefix = prefix.to_uppercase();
    let (trd_market, sec_market) = match prefix.as_str() {
        "US" => (
            crate::pb::trd_common::TrdMarket::Us as i32,
            crate::pb::trd_common::TrdSecMarket::Us as i32,
        ),
        "HK" => (
            crate::pb::trd_common::TrdMarket::Hk as i32,
            crate::pb::trd_common::TrdSecMarket::Hk as i32,
        ),
        "SH" => (
            crate::pb::trd_common::TrdMarket::Cn as i32,
            crate::pb::trd_common::TrdSecMarket::CnSh as i32,
        ),
        "SZ" => (
            crate::pb::trd_common::TrdMarket::Cn as i32,
            crate::pb::trd_common::TrdSecMarket::CnSz as i32,
        ),
        _ => (
            crate::pb::trd_common::TrdMarket::Us as i32,
            crate::pb::trd_common::TrdSecMarket::Us as i32,
        ),
    };

    Ok((trd_market, sec_market, code.to_string()))
}

fn format_trd_symbol(trd_market: Option<i32>, sec_market: Option<i32>, code: Option<&str>) -> String {
    let code = code.unwrap_or_default();
    let prefix = match (trd_market.unwrap_or(0), sec_market.unwrap_or(0)) {
        (x, _) if x == crate::pb::trd_common::TrdMarket::Us as i32 => "US",
        (x, _) if x == crate::pb::trd_common::TrdMarket::Hk as i32 => "HK",
        (x, sm)
            if x == crate::pb::trd_common::TrdMarket::Cn as i32
                && sm == crate::pb::trd_common::TrdSecMarket::CnSh as i32 =>
        {
            "SH"
        }
        (x, sm)
            if x == crate::pb::trd_common::TrdMarket::Cn as i32
                && sm == crate::pb::trd_common::TrdSecMarket::CnSz as i32 =>
        {
            "SZ"
        }
        _ => "UNKNOWN",
    };
    format!("{prefix}.{code}")
}
