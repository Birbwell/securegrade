use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
enum TestStatus {
    Pass,
    Fail,
    PubFail(String, String, String),
    Err(String),
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ResponseObject {
    tests: Vec<(String, TestStatus)>,
}

impl ResponseObject {
    pub fn pass(&mut self, test_name: impl Into<String>) {
        self.tests.push((test_name.into(), TestStatus::Pass));
    }

    pub fn fail(&mut self, test_name: impl Into<String>) {
        self.tests.push((test_name.into(), TestStatus::Fail));
    }

    pub fn pub_fail(
        &mut self,
        test_name: impl Into<String>,
        input: impl Into<String>,
        expected: impl Into<String>,
        found: impl Into<String>,
    ) {
        self.tests.push((
            test_name.into(),
            TestStatus::PubFail(input.into(), expected.into(), found.into()),
        ));
    }

    pub fn err(&mut self, test_name: impl Into<String>, error_msg: impl Into<String>) {
        self.tests
            .push((test_name.into(), TestStatus::Err(error_msg.into())));
    }
}
