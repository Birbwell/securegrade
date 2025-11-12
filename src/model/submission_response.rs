use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Test {
    test_name: String,
    status: String,
    input_output: Option<InputOutput>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SubmissionResponse {
    tests: Vec<Test>,
    passes: usize,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct InputOutput {
    input: String,
    expected: String,
    found: String,
}

impl SubmissionResponse {
    pub fn pass(&mut self, test_name: Option<impl Into<String>>, was_late: bool) {
        self.tests.push(Test {
            test_name: test_name.and_then(|f| Some(f.into())).unwrap_or("".into()),
            status: if was_late { "LATE".into() } else { "PASS".into() },
            input_output: None,
        });
        self.passes += 1;
    }

    pub fn pub_pass(
        &mut self,
        test_name: Option<impl Into<String>>,
        was_late: bool,
        input: impl Into<String>,
        expected: impl Into<String>,
        found: impl Into<String>,
    ) {
        self.tests.push(Test {
            test_name: test_name.and_then(|f| Some(f.into())).unwrap_or("".into()),
            status: if was_late { "LATE".into() } else { "PASS".into() },
            input_output: Some(InputOutput {
                input: input.into(),
                expected: expected.into(),
                found: found.into(),
            }),
        });
        self.passes += 1;
    }

    pub fn fail(&mut self, test_name: Option<impl Into<String>>) {
        // self.tests.push((test_name.into(), TestStatus::Fail));
        self.tests.push(Test {
            test_name: test_name.and_then(|f| Some(f.into())).unwrap_or("".into()),
            status: "FAIL".into(),
            input_output: None,
        })
    }

    pub fn pub_fail(
        &mut self,
        test_name: Option<impl Into<String>>,
        input: impl Into<String>,
        expected: impl Into<String>,
        found: impl Into<String>,
    ) {
        self.tests.push(Test {
            test_name: test_name.and_then(|f| Some(f.into())).unwrap_or("".into()),
            status: "FAIL".into(),
            input_output: Some(InputOutput {
                input: input.into(),
                expected: expected.into(),
                found: found.into(),
            }),
        });
    }

    pub fn time_out(&mut self, test_name: Option<impl Into<String>>) {
        // self.tests.push((test_name.into(), TestStatus::TimeOut));
        self.tests.push(Test {
            test_name: test_name.and_then(|f| Some(f.into())).unwrap_or("".into()),
            status: "TIMED OUT".into(),
            input_output: None,
        })
    }

    pub fn pub_time_out(
        &mut self,
        test_name: Option<impl Into<String>>,
        input: impl Into<String>,
        expected: impl Into<String>,
    ) {
        self.tests.push(Test {
            test_name: test_name.and_then(|f| Some(f.into())).unwrap_or("".into()),
            status: "TIMED OUT".into(),
            input_output: Some(InputOutput {
                input: input.into(),
                expected: expected.into(),
                found: "".into(),
            }),
        });
    }

    pub fn err(&mut self, test_name: Option<impl Into<String>>) {
        self.tests.push(Test {
            test_name: test_name.and_then(|f| Some(f.into())).unwrap_or("".into()),
            status: "ERR".into(),
            input_output: None,
        })
    }

    pub fn pub_err(
        &mut self,
        test_name: Option<impl Into<String>>,
        input: impl Into<String>,
        expected: impl Into<String>,
        found: impl Into<String>,
    ) {
        self.tests.push(Test {
            test_name: test_name.and_then(|f| Some(f.into())).unwrap_or("".into()),
            status: "ERR".into(),
            input_output: Some(InputOutput {
                input: input.into(),
                expected: expected.into(),
                found: found.into(),
            }),
        });
    }

    pub fn score(&self) -> f32 {
        self.passes as f32 / self.tests.len() as f32
    }
}
