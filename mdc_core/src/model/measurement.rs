pub struct Measurement {
    pub id: u64,
    pub operator_name: &'static str,
    pub value: f64,
    pub observation_id: u64, // Ссылка на исходное наблюдение
}