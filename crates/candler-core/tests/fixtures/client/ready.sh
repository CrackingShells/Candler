# Hand-written stub of an evaluator: POSIX sh, three items A1 A2 A3.
read -r request
id=$(printf "%s" "$request" | sed -n "s/.*\"id\":\"\([^\"]*\)\".*/\1/p")
H=abababababababababababababababababababababababababababababababab
outcome() {
  printf "%s\n" "{\"jsonrpc\":\"2.0\",\"method\":\"outcome\",\"params\":{\"record\":{\"run\":{\"cell\":\"sha256:$H\",\"replicate\":1,\"attempt\":1},\"expectation_id\":\"$1\",\"role\":\"gate\",\"evaluator\":\"stub@0.1.0\",\"instruments\":[],\"suite_entry_digest\":\"sha256:$H\",\"context_digest\":\"sha256:$H\",\"request_id\":\"$id\",\"status\":\"pass\",\"weight\":{\"num\":1,\"den\":1},\"evidence\":[]}}}"
}
result() { printf "%s\n" "{\"jsonrpc\":\"2.0\",\"id\":\"$id\",\"result\":{\"reported\":$1}}"; }
# Answers everything after 0.3 s and exits: the answer waits in the pipe, unread, while a test
# blocks the runtime past the request deadline.
sleep 0.3
outcome A1
outcome A2
outcome A3
result 3
