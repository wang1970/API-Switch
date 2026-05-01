use crate::database::{lock_conn, Database};
use crate::error::AppError;
use rusqlite::params_from_iter;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLog {
    pub id: i64,
    pub r#type: i32,
    pub content: String,
    pub access_key_id: Option<String>,
    pub access_key_name: String,
    pub token_name: String,
    pub api_entry_id: String,
    pub channel_id: String,
    pub channel_name: String,
    pub model: String,
    pub requested_model: String,
    pub quota: i64,
    pub is_stream: bool,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub latency_ms: i64,
    pub first_token_ms: i64,
    pub use_time: i64,
    pub status_code: i32,
    pub success: bool,
    pub request_id: String,
    pub log_group: String,
    pub other: String,
    pub error_message: Option<String>,
    pub ip: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageLogFilter {
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub model: Option<String>,
    pub request_id: Option<String>,
    pub channel_id: Option<String>,
    pub access_key_id: Option<String>,
    pub success: Option<bool>,
    pub page: Option<i32>,
    pub page_size: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i32,
    pub page_size: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_requests: i64,
    pub today_requests: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub today_prompt_tokens: i64,
    pub today_completion_tokens: i64,
    pub rpm: f64,
    pub tpm: f64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDataPoint {
    pub time: String,
    pub model: String,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRanking {
    pub model: String,
    pub count: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRanking {
    pub access_key_name: String,
    pub count: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

impl Database {
    pub fn insert_usage_log(
        &self,
        log_type: i32,
        content: &str,
        access_key_id: Option<&str>,
        access_key_name: &str,
        token_name: &str,
        api_entry_id: &str,
        channel_id: &str,
        channel_name: &str,
        model: &str,
        requested_model: &str,
        quota: i64,
        is_stream: bool,
        prompt_tokens: i64,
        completion_tokens: i64,
        latency_ms: i64,
        first_token_ms: i64,
        use_time: i64,
        status_code: i32,
        success: bool,
        request_id: &str,
        log_group: &str,
        other: &str,
        error_message: Option<&str>,
        ip: Option<&str>,
    ) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO usage_logs (type, content, access_key_id, access_key_name, token_name, api_entry_id, channel_id, channel_name,
             model, requested_model, quota, is_stream, prompt_tokens, completion_tokens, latency_ms, first_token_ms,
             use_time, status_code, success, request_id, log_group, other, error_message, ip, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)",
            rusqlite::params![
                log_type, content, access_key_id, access_key_name, token_name, api_entry_id, channel_id, channel_name,
                model, requested_model, quota, is_stream as i32, prompt_tokens, completion_tokens,
                latency_ms, first_token_ms, use_time, status_code, success as i32,
                request_id, log_group, other, error_message, ip, now
            ],
        )?;

        Ok(())
    }

    pub fn get_usage_logs(
        &self,
        filter: &UsageLogFilter,
    ) -> Result<PaginatedResult<UsageLog>, AppError> {
        let conn = lock_conn!(self.conn);

        let page = filter.page.unwrap_or(1).max(1);
        let page_size = filter.page_size.unwrap_or(50).min(200);

        let mut where_clauses = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(start) = filter.start_time {
            where_clauses.push(format!("created_at >= ?{}", params.len() + 1));
            params.push(Box::new(start));
        }
        if let Some(end) = filter.end_time {
            where_clauses.push(format!("created_at <= ?{}", params.len() + 1));
            params.push(Box::new(end));
        }
        if let Some(ref model) = filter.model {
            where_clauses.push(format!(
                "(model LIKE ?{} OR requested_model LIKE ?{})",
                params.len() + 1,
                params.len() + 2
            ));
            params.push(Box::new(format!("%{model}%")));
            params.push(Box::new(format!("%{model}%")));
        }
        if let Some(ref request_id) = filter.request_id {
            where_clauses.push(format!("request_id = ?{}", params.len() + 1));
            params.push(Box::new(request_id.clone()));
        }
        if let Some(ref channel_id) = filter.channel_id {
            where_clauses.push(format!("channel_id = ?{}", params.len() + 1));
            params.push(Box::new(channel_id.clone()));
        }
        if let Some(ref access_key_id) = filter.access_key_id {
            where_clauses.push(format!("access_key_id = ?{}", params.len() + 1));
            params.push(Box::new(access_key_id.clone()));
        }
        if let Some(success) = filter.success {
            where_clauses.push(format!("success = ?{}", params.len() + 1));
            params.push(Box::new(success as i32));
        }

        let where_str = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        // Count
        let count_sql = format!("SELECT COUNT(*) FROM usage_logs {where_str}");
        let total: i64 = conn.query_row(&count_sql, params_from_iter(params.iter()), |row| {
            row.get(0)
        })?;

        // Query
        let offset = (page - 1) * page_size;
        let query_sql = format!(
            "SELECT id, type, content, access_key_id, access_key_name, token_name, api_entry_id, channel_id, channel_name,
                    model, requested_model, quota, is_stream, prompt_tokens, completion_tokens,
                    latency_ms, first_token_ms, use_time, status_code, success, request_id, log_group, other, error_message, ip, created_at
             FROM usage_logs {where_str} ORDER BY created_at DESC LIMIT ?{} OFFSET ?{}",
            params.len() + 1,
            params.len() + 2
        );
        params.push(Box::new(page_size));
        params.push(Box::new(offset));

        let mut stmt = conn.prepare(&query_sql)?;
        let items = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                let is_stream: i32 = row.get(12)?;
                let success: i32 = row.get(19)?;
                Ok(UsageLog {
                    id: row.get(0)?,
                    r#type: row.get(1)?,
                    content: row.get(2)?,
                    access_key_id: row.get(3)?,
                    access_key_name: row.get(4)?,
                    token_name: row.get(5)?,
                    api_entry_id: row.get(6)?,
                    channel_id: row.get(7)?,
                    channel_name: row.get(8)?,
                    model: row.get(9)?,
                    requested_model: row.get(10)?,
                    quota: row.get(11)?,
                    is_stream: is_stream != 0,
                    prompt_tokens: row.get(13)?,
                    completion_tokens: row.get(14)?,
                    latency_ms: row.get(15)?,
                    first_token_ms: row.get(16)?,
                    use_time: row.get(17)?,
                    status_code: row.get(18)?,
                    success: success != 0,
                    request_id: row.get(20)?,
                    log_group: row.get(21)?,
                    other: row.get(22)?,
                    error_message: row.get(23)?,
                    ip: row.get(24)?,
                    created_at: row.get(25)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(PaginatedResult {
            items,
            total,
            page,
            page_size,
        })
    }

    pub fn get_dashboard_stats(
        &self,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<DashboardStats, AppError> {
        let conn = lock_conn!(self.conn);
        let now = chrono::Local::now();
        let now_ts = now.timestamp();
        let today_start = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .and_then(|start| start.and_local_timezone(chrono::Local).single())
            .map(|start| start.timestamp())
            .unwrap_or_else(|| now_ts - (now_ts % 86400)); // Fallback to UTC day boundary if local midnight is ambiguous.
        let effective_start = start_time.unwrap_or(0);
        let effective_end = end_time.unwrap_or(now_ts);

        // Total stats
        let (total_requests, total_prompt, total_completion, total_success, total_latency): (i64, i64, i64, i64, i64) =
            conn.query_row(
                "SELECT COUNT(*), COALESCE(SUM(prompt_tokens),0), COALESCE(SUM(completion_tokens),0),
                         COALESCE(SUM(CASE WHEN success=1 THEN 1 ELSE 0 END),0), COALESCE(SUM(latency_ms),0)
                 FROM usage_logs
                 WHERE created_at >= ?1 AND created_at <= ?2",
                [effective_start, effective_end],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )?;

        // Today stats
        let (today_requests, today_prompt, today_completion): (i64, i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(prompt_tokens),0), COALESCE(SUM(completion_tokens),0)
                 FROM usage_logs WHERE created_at >= ?1 AND created_at <= ?2 AND created_at >= ?3",
            [effective_start, effective_end, today_start],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        // RPM/TPM (last 60 seconds)
        let minute_ago = now_ts - 60;
        let (rpm, tpm): (f64, f64) = conn.query_row(
            "SELECT COUNT(*) as rpm, COALESCE(SUM(prompt_tokens) + SUM(completion_tokens), 0) as tpm
             FROM usage_logs WHERE created_at >= ?1",
            [minute_ago],
            |row| {
                let r: i64 = row.get(0)?;
                let t: i64 = row.get(1)?;
                Ok((r as f64, t as f64))
            },
        )?;

        let success_rate = if total_requests > 0 {
            total_success as f64 / total_requests as f64
        } else {
            1.0
        };

        let avg_latency = if total_requests > 0 {
            total_latency as f64 / total_requests as f64
        } else {
            0.0
        };

        Ok(DashboardStats {
            total_requests,
            today_requests,
            total_prompt_tokens: total_prompt,
            total_completion_tokens: total_completion,
            today_prompt_tokens: today_prompt,
            today_completion_tokens: today_completion,
            rpm,
            tpm,
            success_rate,
            avg_latency_ms: avg_latency,
        })
    }

    pub fn get_model_consumption(
        &self,
        start_time: Option<i64>,
        end_time: Option<i64>,
        granularity: Option<&str>,
    ) -> Result<Vec<ChartDataPoint>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut where_clauses = Vec::new();
        let time_expr = time_bucket_expr(granularity);

        if let Some(start) = start_time {
            where_clauses.push(format!("created_at >= ?{}", params.len() + 1));
            params.push(Box::new(start));
        }
        if let Some(end) = end_time {
            where_clauses.push(format!("created_at <= ?{}", params.len() + 1));
            params.push(Box::new(end));
        }

        let where_str = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let sql = format!(
            "SELECT {time_expr} as time, model,
                    SUM(prompt_tokens + completion_tokens) as value
              FROM usage_logs {where_str}
              GROUP BY time, model ORDER BY time"
        );

        let mut stmt = conn.prepare(&sql)?;
        let data = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                Ok(ChartDataPoint {
                    time: row.get(0)?,
                    model: row.get(1)?,
                    value: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(data)
    }

    pub fn get_call_trend(
        &self,
        start_time: Option<i64>,
        end_time: Option<i64>,
        granularity: Option<&str>,
    ) -> Result<Vec<ChartDataPoint>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut where_clauses = Vec::new();
        let time_expr = time_bucket_expr(granularity);

        if let Some(start) = start_time {
            where_clauses.push(format!("created_at >= ?{}", params.len() + 1));
            params.push(Box::new(start));
        }
        if let Some(end) = end_time {
            where_clauses.push(format!("created_at <= ?{}", params.len() + 1));
            params.push(Box::new(end));
        }

        let where_str = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let sql = format!(
            "SELECT {time_expr} as time, model, COUNT(*) as value
              FROM usage_logs {where_str}
              GROUP BY time, model ORDER BY time"
        );

        let mut stmt = conn.prepare(&sql)?;
        let data = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                Ok(ChartDataPoint {
                    time: row.get(0)?,
                    model: row.get(1)?,
                    value: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(data)
    }

    pub fn get_model_distribution(
        &self,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<ModelRanking>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut where_clauses = Vec::new();

        if let Some(start) = start_time {
            where_clauses.push(format!("created_at >= ?{}", params.len() + 1));
            params.push(Box::new(start));
        }
        if let Some(end) = end_time {
            where_clauses.push(format!("created_at <= ?{}", params.len() + 1));
            params.push(Box::new(end));
        }

        let where_str = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let sql = format!(
            "SELECT model, COUNT(*) as count,
                    COALESCE(SUM(prompt_tokens),0) as prompt_tokens,
                    COALESCE(SUM(completion_tokens),0) as completion_tokens
             FROM usage_logs {where_str}
             GROUP BY model ORDER BY count DESC"
        );

        let mut stmt = conn.prepare(&sql)?;
        let data = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                Ok(ModelRanking {
                    model: row.get(0)?,
                    count: row.get(1)?,
                    prompt_tokens: row.get(2)?,
                    completion_tokens: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(data)
    }

    pub fn get_model_ranking(
        &self,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<ModelRanking>, AppError> {
        // Same as distribution, just ordered differently
        self.get_model_distribution(start_time, end_time)
    }

    pub fn get_user_ranking(
        &self,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<UserRanking>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut where_clauses = Vec::new();

        if let Some(start) = start_time {
            where_clauses.push(format!("created_at >= ?{}", params.len() + 1));
            params.push(Box::new(start));
        }
        if let Some(end) = end_time {
            where_clauses.push(format!("created_at <= ?{}", params.len() + 1));
            params.push(Box::new(end));
        }

        let where_str = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let sql = format!(
            "SELECT access_key_name, COUNT(*) as count,
                    COALESCE(SUM(prompt_tokens),0) as prompt_tokens,
                    COALESCE(SUM(completion_tokens),0) as completion_tokens
             FROM usage_logs {where_str}
             GROUP BY access_key_name ORDER BY count DESC LIMIT 10"
        );

        let mut stmt = conn.prepare(&sql)?;
        let data = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                Ok(UserRanking {
                    access_key_name: row.get(0)?,
                    count: row.get(1)?,
                    prompt_tokens: row.get(2)?,
                    completion_tokens: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(data)
    }

    pub fn get_user_trend(
        &self,
        start_time: Option<i64>,
        end_time: Option<i64>,
        granularity: Option<&str>,
    ) -> Result<Vec<ChartDataPoint>, AppError> {
        let conn = lock_conn!(self.conn);
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut where_clauses = Vec::new();
        let time_expr = time_bucket_expr(granularity);

        if let Some(start) = start_time {
            where_clauses.push(format!("created_at >= ?{}", params.len() + 1));
            params.push(Box::new(start));
        }
        if let Some(end) = end_time {
            where_clauses.push(format!("created_at <= ?{}", params.len() + 1));
            params.push(Box::new(end));
        }

        let where_str = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let sql = format!(
            "SELECT {time_expr} as time, access_key_name as model,
                    SUM(prompt_tokens + completion_tokens) as value
              FROM usage_logs {where_str}
              GROUP BY time, access_key_name ORDER BY time"
        );

        let mut stmt = conn.prepare(&sql)?;
        let data = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                Ok(ChartDataPoint {
                    time: row.get(0)?,
                    model: row.get(1)?,
                    value: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(data)
    }
}

fn time_bucket_expr(granularity: Option<&str>) -> &'static str {
    match granularity {
        Some("hour") => "strftime('%Y-%m-%d %H:00', created_at, 'unixepoch', 'localtime')",
        _ => "date(created_at, 'unixepoch', 'localtime')",
    }
}
