use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};

pub fn get_date_from_parts(year: Option<i32>, month: Option<u32>, day: Option<u32>, hour: Option<u32>, min: Option<u32>, sec: Option<u32>) -> DateTime<Utc> {
    Utc.from_utc_datetime(
        &NaiveDateTime::new(
            NaiveDate::from_ymd_opt(
                year.unwrap_or(1900),
                month.unwrap_or(1), 
                day.unwrap_or(1)
            ).unwrap_or(NaiveDate::MIN),
            NaiveTime::from_hms_opt(
                hour.unwrap_or(0),
                min.unwrap_or(0),
                sec.unwrap_or(0)
            ).unwrap_or(NaiveTime::MIN)
        )
    )
}


pub fn get_first_date() -> DateTime<Utc> {
    get_date_from_parts(None, None, None, None, None, None)
}


pub fn get_datetime() -> DateTime<Utc> {
    Utc::now()
}


pub fn is_min_date(datetime: &DateTime<Utc>) -> bool {
    *datetime == Utc.from_utc_datetime(&NaiveDateTime::MIN)
}
