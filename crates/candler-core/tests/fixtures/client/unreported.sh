# Hand-written stub of an evaluator: POSIX sh, three items A1 A2 A3.
read -r request
id=$(printf "%s" "$request" | sed -n "s/.*\"id\":\"\([^\"]*\)\".*/\1/p")
H=abababababababababababababababababababababababababababababababab
outcome() {
  printf "%s\n" "{\"jsonrpc\":\"2.0\",\"method\":\"outcome\",\"params\":{\"record\":{\"run\":{\"cell\":\"sha256:$H\",\"replicate\":1,\"attempt\":1},\"expectation_id\":\"$1\",\"role\":\"gate\",\"evaluator\":\"stub@0.1.0\",\"instruments\":[],\"suite_entry_digest\":\"sha256:$H\",\"context_digest\":\"sha256:$H\",\"request_id\":\"$id\",\"status\":\"pass\",\"weight\":{\"num\":1,\"den\":1},\"evidence\":[]}}}"
}
result() { printf "%s\n" "{\"jsonrpc\":\"2.0\",\"id\":\"$id\",\"result\":{\"reported\":$1}}"; }
# Reports A1, then exits without a result: A2 and A3 are never reported.
outcome A1
