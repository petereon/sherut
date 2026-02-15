Describe 'sherut basic functionality'
  # Start server before all tests in this spec
  setup() {
    start_sherut \
      --route "GET /hello" "echo 'Hello, World!'" \
      --route "GET /hello/:name" "echo 'Hello, :name!'" \
      --route "POST /echo" "cat"
  }

  # Stop server after all tests
  cleanup() {
    stop_sherut 2>/dev/null
  }

  BeforeAll 'setup'
  AfterAll 'cleanup'

  Describe 'GET /hello'
    It 'returns hello world'
      When call http_get "/hello"
      The output should equal "Hello, World!"
      The status should be success
    End

    It 'returns 200 status'
      When call http_get_status "/hello"
      The output should equal "200"
    End
  End

  Describe 'GET /hello/:name'
    It 'says hello to Alice'
      When call http_get "/hello/Alice"
      The output should equal "Hello, Alice!"
    End

    It 'says hello to Bob'
      When call http_get "/hello/Bob"
      The output should equal "Hello, Bob!"
    End
  End

  Describe 'POST /echo'
    It 'echoes the request body'
      When call http_post "/echo" "test data"
      The output should equal "test data"
    End
  End

  Describe '404 handling'
    It 'returns 404 for unknown routes'
      When call http_get_status "/unknown"
      The output should equal "404"
    End
  End
End
