use serde::{Deserialize, Serialize};

// #[derive(Debug, Serialize, Deserialize)]
// enum TestStatus {
//     Pass,
//     Fail,
//     PubFail {
//         input: String,
//         expected: String,
//         found: String,
//     },
//     TimeOut,
//     Err(String),
// }

// #[derive(Debug, Default, Serialize, Deserialize)]
// pub struct SubmissionResponse {
//     tests: Vec<(String, TestStatus)>,
// }

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Test {
    test_name: String,
    status: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SubmissionResponse {
    tests: Vec<Test>,
    passes: usize,
}

impl SubmissionResponse {
    pub fn pass(&mut self, test_name: impl Into<String>) {
        // self.tests.push((test_name.into(), TestStatus::Pass));
        self.tests.push(Test {
            test_name: test_name.into(),
            status: "PASS".into(),
        });
        self.passes += 1;
    }

    pub fn fail(&mut self, test_name: impl Into<String>) {
        // self.tests.push((test_name.into(), TestStatus::Fail));
        self.tests.push(Test {
            test_name: test_name.into(),
            status: "FAIL".into(),
        })
    }

    pub fn time_out(&mut self, test_name: impl Into<String>) {
        // self.tests.push((test_name.into(), TestStatus::TimeOut));
        self.tests.push(Test {
            test_name: test_name.into(),
            status: "TIMED OUT".into(),
        })
    }

    // pub fn pub_fail(
    //     &mut self,
    //     test_name: impl Into<String>,
    //     input: impl Into<String>,
    //     expected: impl Into<String>,
    //     found: impl Into<String>,
    // ) {
    //     self.tests.push((
    //         test_name.into(),
    //         TestStatus::PubFail {
    //             input: input.into(),
    //             expected: expected.into(),
    //             found: found.into(),
    //         },
    //     ));
    // }

    pub fn err(&mut self, test_name: impl Into<String>, error_msg: impl Into<String>) {
        // self.tests
        // .push((test_name.into(), TestStatus::Err(error_msg.into())));

        self.tests.push(Test {
            test_name: test_name.into(),
            status: format!("Err: {}", error_msg.into()),
        })
    }

    pub fn score(&self) -> f32 {
        self.passes as f32 / self.tests.len() as f32
    }
}
