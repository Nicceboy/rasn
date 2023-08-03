TEST_EXECUTABLE="$(cargo t --no-run -q --lib --message-format=json  oer::enc::tests::test_encode_integer_manual_setup | jq -r '[inputs][-2].executable')"
rust-lldb $TEST_EXECUTABLE
