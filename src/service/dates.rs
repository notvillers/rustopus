use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};

/// This function returns `DateTime<Utc>` from the given parts, similar to `DATETIMEFROMPARTS()` in MSSQL
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


/// This function gets the minimum date can be passed to a request in `DateTime<Utc>`
pub fn get_first_date() -> DateTime<Utc> {
    get_date_from_parts(None, None, None, None, None, None)
}


/// This function returns now in `DateTime<Utc>`
pub fn get_datetime() -> DateTime<Utc> {
    Utc::now()
}


/// This function checks if the `DateTime<Utc>` is naive minimum, because a low date like this can not be passed as a request
pub fn is_min_date(datetime: &DateTime<Utc>) -> bool {
    *datetime == Utc.from_utc_datetime(&NaiveDateTime::MIN)
}
