use super::utility::get_json;
use crate::types::Result;
use reqwest::{Client, Url};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::try_join;
use tracing::{error, info, instrument};

pub struct Notify {
    tp: u8,
    is_selected: bool,
    /// 删除系统通知的两种api
    system_notify_api: Option<u8>,
}
impl Notify {
    pub fn new(tp: u8) -> Notify {
        Notify {
            tp,
            is_selected: false,
            system_notify_api: None,
        }
    }

    fn new_system_notify(tp: u8, api_type: u8) -> Notify {
        Notify {
            tp,
            is_selected: false,
            system_notify_api: Some(api_type),
        }
    }
    #[instrument(skip_all)]
    pub async fn remove(&self, id: u64, cl: Arc<Client>, csrf: Arc<String>) -> Result<u64> {
        match self.system_notify_api {
            Some(api_type) => {
                let json = if api_type == 0 {
                    json!({"csrf":*csrf,"ids":[id],"station_ids":[],"type":self.tp,"build":8140300,"mobi_app":"android"})
                } else {
                    json!({"csrf":*csrf,"ids":[],"station_ids":[id],"type":self.tp,"build":8140300,"mobi_app":"android"})
                };
                let json_res:Value = cl
                    .post(
                        format!("https://message.bilibili.com/x/sys-msg/del_notify_list?build=8140300&mobi_app=android&csrf={csrf}"),
                    )
                    .json(&json)
                    .send()
                    .await
                    ?
                    .json()
                    .await
                    ?;
                if json_res["code"].as_i64().unwrap() == 0 {
                    info!("Remove system notify {id} successfully");
                    Ok(id)
                } else {
                    let e = format!(
                        "Can't remove the system notify. Response json: {}",
                        json_res
                    );
                    error!(e);
                    Err(e.into())
                }
            }
            None => {
                let json_res: Value = cl
                    .post(
                        "
    https://api.bilibili.com/x/msgfeed/del",
                    )
                    .form(&[
                        ("tp", self.tp.to_string()),
                        ("id", id.to_string()),
                        ("build", 0.to_string()),
                        ("mobi_app", "web".to_string()),
                        ("csrf_token", csrf.to_string()),
                        ("csrf", csrf.to_string()),
                    ])
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                if json_res["code"]
                    .as_i64()
                    .ok_or("Remove Notify: Parse json res code failed")?
                    == 0
                {
                    info!("Remove notify {} successfully", id);
                    Ok(id)
                } else {
                    let e = format!("Can't remove notify. Response json: {}", json_res);
                    error!(e);
                    Err(e.into())
                }
            }
        }
    }

    pub async fn fetch(cl: Arc<Client>, csrf: Arc<String>) -> Result<HashMap<u64, Self>> {
        let a = try_join!(
            Self::fetch_liked_notify(cl.clone()),
            Self::fetch_ated_notify(cl.clone()),
            Self::fetch_replyed_notify(cl.clone()),
            Self::fetch_system_notify(cl.clone(), csrf.clone())
        )?;
        let (m1, m2, m3, m4) = a;
        Ok(m1.into_iter().chain(m2).chain(m3).chain(m4).collect())
    }
    #[instrument(skip_all)]
    pub async fn fetch_liked_notify(cl: Arc<Client>) -> Result<HashMap<u64, Self>> {
        let mut h: HashMap<u64, Self> = HashMap::new();
        let mut queryid = None;
        let mut last_time = None;

        loop {
            let json: serde_json::Value;
            let notifys: &serde_json::Value;
            // first get
            if queryid.is_none() && last_time.is_none() {
                json = get_json(
                    cl.clone(),
                    "https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web",
                )
                .await?;
                notifys = &json["data"]["total"]["items"];
                if notifys.as_array().unwrap().is_empty() {
                    let i = "没有收到赞的通知。";
                    info!(i);
                    return Err(i.into());
                }
                last_time = notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                queryid = json["data"]["total"]["cursor"]["id"].as_u64();
            } else {
                json=get_json(cl.clone(), format!("https://api.bilibili.com/x/msgfeed/like?platform=web&build=0&mobi_app=web&id={}&like_time={}",&queryid.unwrap().to_string(),&last_time.unwrap().to_string())).await?;
                notifys = &json["data"]["total"]["items"];
                last_time = notifys.as_array().unwrap().last().unwrap()["like_time"].as_u64();
                queryid = json["data"]["total"]["cursor"]["id"].as_u64();
            }

            for i in notifys.as_array().unwrap() {
                let notify_id = i["id"].as_u64().unwrap();
                h.insert(notify_id, Notify::new(0));
                info!("Fetched notify {notify_id}");
            }

            if json["data"]["total"]["cursor"]["is_end"].as_bool().unwrap() {
                info!("收到赞的通知处理完毕。通知数量：{}", h.len());
                break;
            }
        }
        Ok(h)
    }

    #[instrument(skip_all)]
    pub async fn fetch_replyed_notify(cl: Arc<Client>) -> Result<HashMap<u64, Self>> {
        let mut h: HashMap<u64, Self> = HashMap::new();
        let mut queryid = None;
        let mut last_time = None;

        loop {
            let json: serde_json::Value;
            let notifys: &serde_json::Value;
            // first get
            if queryid.is_none() && last_time.is_none() {
                json = get_json(
                    cl.clone(),
                    "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web",
                )
                .await?;
                notifys = &json["data"]["items"];
                if notifys.as_array().unwrap().is_empty() {
                    let i = "没有收到评论的通知。";
                    info!(i);
                    return Err(i.into());
                }
                last_time = notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                queryid = json["data"]["cursor"]["id"].as_u64();
            } else {
                let mut url = Url::parse(
                    "https://api.bilibili.com/x/msgfeed/reply?platform=web&build=0&mobi_app=web",
                )
                .unwrap();
                url.query_pairs_mut()
                    .append_pair("id", &queryid.unwrap().to_string())
                    .append_pair("reply_time", &last_time.unwrap().to_string());
                json = get_json(cl.clone(), url).await?;
                notifys = &json["data"]["items"];
                last_time = notifys.as_array().unwrap().last().unwrap()["reply_time"].as_u64();
                queryid = json["data"]["cursor"]["id"].as_u64();
            }

            for i in notifys.as_array().unwrap() {
                let notify_id = i["id"].as_u64().unwrap();
                h.insert(notify_id, Notify::new(1));
                info!("Fetched notify {notify_id}");
            }

            if json["data"]["cursor"]["is_end"].as_bool().unwrap() {
                info!("收到评论的通知处理完毕。通知数量：{}", h.len());
                break;
            }
        }
        Ok(h)
    }
    #[instrument(skip_all)]
    pub async fn fetch_ated_notify(cl: Arc<Client>) -> Result<HashMap<u64, Self>> {
        let mut h: HashMap<u64, Self> = HashMap::new();
        let mut queryid = None;
        let mut last_time = None;

        loop {
            let json: serde_json::Value;
            let notifys: &serde_json::Value;
            // first get
            if queryid.is_none() && last_time.is_none() {
                json = get_json(
                    cl.clone(),
                    "https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web",
                )
                .await?;
                notifys = &json["data"]["items"];
                if notifys.as_array().unwrap().is_empty() {
                    let i = "没有被At的通知。";
                    info!(i);
                    return Err(i.into());
                }
                last_time = notifys.as_array().unwrap().last().unwrap()["at_time"].as_u64();
                queryid = json["data"]["cursor"]["id"].as_u64();
            } else {
                let mut url =
                    Url::parse("https://api.bilibili.com/x/msgfeed/at?build=0&mobi_app=web")
                        .unwrap();
                url.query_pairs_mut()
                    .append_pair("id", &queryid.unwrap().to_string())
                    .append_pair("at_time", &last_time.unwrap().to_string());
                json = get_json(cl.clone(), url).await?;
                notifys = &json["data"]["items"];
                last_time = notifys.as_array().unwrap().last().unwrap()["at_time"].as_u64();
                queryid = json["data"]["cursor"]["id"].as_u64();
            }

            for i in notifys.as_array().unwrap() {
                let notify_id = i["id"].as_u64().unwrap();
                h.insert(notify_id, Notify::new(2));
                info!("Fetched notify {notify_id}");
            }

            if json["data"]["cursor"]["is_end"].as_bool().unwrap() {
                info!("被At的通知处理完毕。通知数量：{}", h.len());
                break;
            }
        }
        Ok(h)
    }

    #[instrument(skip_all)]
    pub async fn fetch_system_notify(
        cl: Arc<Client>,
        csrf: Arc<String>,
    ) -> Result<HashMap<u64, Self>> {
        let mut h: HashMap<u64, Self> = HashMap::new();
        let mut cursor = None;
        let mut api_type = 0_u8;
        loop {
            let mut json: serde_json::Value;
            let mut notifys: &serde_json::Value;
            // first get
            if cursor.is_none() {
                json = get_json(
                    cl.clone(),
                    format!("https://message.bilibili.com/x/sys-msg/query_user_notify?csrf={csrf}&csrf={csrf}&page_size=20&build=0&mobi_app=web"),
                )
                    .await?;
                notifys = &json["data"]["system_notify_list"];
                // 第一种api（0）获取为空时
                if notifys.is_null() {
                    api_type = 1;
                    json = get_json(cl.clone(), format!("https://message.bilibili.com/x/sys-msg/query_unified_notify?csrf={csrf}&csrf={csrf}&page_size=10&build=0&mobi_app=web")).await?;
                    notifys = &json["data"]["system_notify_list"];
                    // 两者都为空
                    if notifys.is_null() {
                        let i = "没有系统通知。";
                        info!(i);
                        return Err(i.into());
                    }
                }
                cursor = notifys.as_array().unwrap().last().unwrap()["cursor"].as_u64();
            } else {
                let url =
                    format!("https://message.bilibili.com/x/sys-msg/query_notify_list?csrf={csrf}&data_type=1&cursor={}&build=0&mobi_app=web",cursor.unwrap());
                json = get_json(cl.clone(), url).await?;
                notifys = &json["data"];
                if json["data"].as_array().unwrap().is_empty() {
                    info!("系统通知处理完毕。通知数量：{}", h.len());
                    break;
                }
                cursor = notifys.as_array().unwrap().last().unwrap()["cursor"].as_u64();
            }

            for i in notifys.as_array().unwrap() {
                let notify_id = i["id"].as_u64().unwrap();
                let notify_type = i["type"].as_u64().unwrap() as u8;
                h.insert(notify_id, Notify::new_system_notify(notify_type, api_type));
                info!("Fetched notify {notify_id}");
            }
        }
        Ok(h)
    }
}
