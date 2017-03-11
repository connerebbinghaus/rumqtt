use std::net::{SocketAddr, ToSocketAddrs};
use std::str;
use std::sync::Arc;
use std::thread;
use std::sync::mpsc::{sync_channel, SyncSender};

use mqtt3::{QoS, TopicPath, Message};

use error::{Result, Error};
use clientoptions::MqttOptions;
use connection::{Connection, NetworkRequest};
use callbacks::MqttCallback;

use std::time::Duration;
use std::sync::mpsc::TrySendError;

pub struct MqttClient {
    pub nw_request_tx: SyncSender<NetworkRequest>,
}

impl MqttClient {
    fn lookup_ipv4<A: ToSocketAddrs>(addr: A) -> SocketAddr {
        let addrs = addr.to_socket_addrs().expect("Conversion Failed");
        for addr in addrs {
            if let SocketAddr::V4(_) = addr {
                return addr;
            }
        }
        unreachable!("Cannot lookup address");
    }

    pub fn publish(&mut self, topic: &str, qos: QoS, payload: Vec<u8>) -> Result<()> {
        let payload = Arc::new(payload);
        let mut ret_val;
        loop {
            let payload = payload.clone();
            ret_val = self._publish(topic, false, qos, payload, None);
            if let Err(Error::TrySend(ref e)) = ret_val {
                match e {
                    // break immediately if rx is dropped
                    &TrySendError::Disconnected(_) => break,
                    &TrySendError::Full(_) => {
                        warn!("Request Queue Full !!!!!!!!");
                        thread::sleep(Duration::new(2, 0));
                        continue;
                    }
                }
            } else {
                return ret_val;
            }
        }
        ret_val
    }

    pub fn retained_publish(&mut self, topic: &str, qos: QoS, payload: Vec<u8>) -> Result<()> {
        let payload = Arc::new(payload);
        self._publish(topic, true, qos, payload, None)
    }

    pub fn userdata_publish(&mut self, topic: &str, qos: QoS, payload: Vec<u8>, userdata: Vec<u8>) -> Result<()> {
        let payload = Arc::new(payload);
        let userdata = Arc::new(userdata);
        let mut ret_val;
        loop {
            let payload = payload.clone();
            ret_val = self._publish(topic, false, qos, payload, Some(userdata.clone()));
            if let Err(Error::TrySend(ref e)) = ret_val {
                match e {
                    // break immediately if rx is dropped
                    &TrySendError::Disconnected(_) => break,
                    &TrySendError::Full(_) => {
                        warn!("Request Queue Full !!!!!!!!");
                        thread::sleep(Duration::new(2, 0));
                        continue;
                    }
                }
            } else {
                return ret_val;
            }
        }
        ret_val
    }

    pub fn retained_userdata_publish(&mut self, topic: &str, qos: QoS, payload: Vec<u8>, userdata: Vec<u8>) -> Result<()> {
        let payload = Arc::new(payload);
        let userdata = Arc::new(userdata);
        self._publish(topic, true, qos, payload, Some(userdata))
    }

    pub fn disconnect(&self) -> Result<()> {
        self.nw_request_tx.send(NetworkRequest::Disconnect)?;
        Ok(())
    }

    pub fn shutdown(&self) -> Result<()> {
        self.nw_request_tx.send(NetworkRequest::Shutdown)?;
        Ok(())
    }

    fn _publish(&mut self,
                topic: &str,
                retain: bool,
                qos: QoS,
                payload: Arc<Vec<u8>>,
                userdata: Option<Arc<Vec<u8>>>)
                -> Result<()> {

        let topic = TopicPath::from_str(topic.to_string())?;

        let message = Message {
            topic: topic,
            retain: retain,
            qos: qos,
            payload: payload,
            pid: None,
        };
        let message = Box::new(message);

        // TODO: Check message sanity here and return error if not
        match qos {
            QoS::AtMostOnce | QoS::AtLeastOnce | QoS::ExactlyOnce => {
                self.nw_request_tx.try_send(NetworkRequest::Publish(message))?
            }
        };

        Ok(())
    }
}

// @@@@@@@@@@@@~~~~~~~UNIT TESTS ~~~~~~~~~@@@@@@@@@@@@

#[cfg(test)]
mod test {
    extern crate env_logger;

    use mqtt3::QoS;
    use clientoptions::MqttOptions;
    use super::MqttClient;
    use error::Result;

    use std::sync::Arc;
    use std::thread;
    use std::sync::mpsc::sync_channel;
    use connection::NetworkRequest;
    use std::time::Duration;

    fn mock_start(_: MqttOptions, forever: bool) -> Result<MqttClient> {
        let (nw_request_tx, nw_request_rx) = sync_channel::<NetworkRequest>(50);

        thread::spawn(move || -> Result<()> {
            let _ = nw_request_rx;
            if forever {
                thread::sleep(Duration::new(1000_000, 0));
            }
            Ok(())
        });

        let client = MqttClient { nw_request_tx: nw_request_tx };

        Ok(client)
    }

    #[test]
    #[should_panic]
    fn request_queue_blocks_when_buffer_full() {
        env_logger::init().unwrap();
        let client_options = MqttOptions::new().set_broker("test.mosquitto.org:1883");
        match mock_start(client_options, true) {
            Ok(mut mq_client) => {
                for _ in 0..65536 {
                    mq_client._publish("hello/world", false, QoS::Level1, Arc::new(vec![1u8, 2, 3]), None).unwrap();
                }
            }
            Err(e) => panic!("{:?}", e),
        }
    }

    #[test]
    #[should_panic]
    fn publish_should_not_happen_rxdrop() {
        env_logger::init().unwrap();
        let client_options = MqttOptions::new().set_broker("test.mosquitto.org:1883");
        match mock_start(client_options, false) {
            Ok(mut mq_client) => {
                for _ in 0..65536 {
                    mq_client._publish("hello/world", false, QoS::Level1, Arc::new(vec![1u8, 2, 3]), None).unwrap();
                }
            }
            Err(e) => panic!("{:?}", e),
        }
    }
}
