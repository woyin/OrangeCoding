package controlserver

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/woyin/OrangeCoding/modules/control-protocol"
	"github.com/woyin/OrangeCoding/modules/worker"
)

func newTestServer(t *testing.T) (*Server, *httptest.Server) {
	t.Helper()
	eventCh := make(chan controlprotocol.ServerEvent, 64)
	wr := worker.NewWorkerRuntime(eventCh)
	s := NewServer(wr, ":0")
	ts := httptest.NewServer(s.Router())
	return s, ts
}

func TestCreateSession(t *testing.T) {
	_, ts := newTestServer(t)
	defer ts.Close()

	resp, err := http.Post(ts.URL+"/sessions", "application/json", nil)
	if err != nil {
		t.Fatalf("POST /sessions: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		t.Errorf("status = %d, want %d", resp.StatusCode, http.StatusOK)
	}

	var result map[string]interface{}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		t.Fatalf("decode response: %v", err)
	}

	if _, ok := result["session_id"]; !ok {
		t.Error("response missing session_id")
	}
}

func TestListSessions(t *testing.T) {
	_, ts := newTestServer(t)
	defer ts.Close()

	// Create a session first
	resp, err := http.Post(ts.URL+"/sessions", "application/json", nil)
	if err != nil {
		t.Fatalf("POST /sessions: %v", err)
	}
	resp.Body.Close()

	// List sessions
	resp, err = http.Get(ts.URL + "/sessions")
	if err != nil {
		t.Fatalf("GET /sessions: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		t.Errorf("status = %d, want %d", resp.StatusCode, http.StatusOK)
	}

	var result []map[string]interface{}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		t.Fatalf("decode response: %v", err)
	}

	if len(result) < 1 {
		t.Errorf("len(sessions) = %d, want >= 1", len(result))
	}
}

func TestGetSession(t *testing.T) {
	_, ts := newTestServer(t)
	defer ts.Close()

	// Create a session
	resp, err := http.Post(ts.URL+"/sessions", "application/json", nil)
	if err != nil {
		t.Fatalf("POST /sessions: %v", err)
	}
	var createResult map[string]interface{}
	if err := json.NewDecoder(resp.Body).Decode(&createResult); err != nil {
		t.Fatalf("decode create response: %v", err)
	}
	resp.Body.Close()

	sessionID := createResult["session_id"].(string)

	// Get session
	resp, err = http.Get(ts.URL + "/sessions/" + sessionID)
	if err != nil {
		t.Fatalf("GET /sessions/%s: %v", sessionID, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		t.Errorf("status = %d, want %d", resp.StatusCode, http.StatusOK)
	}
}

func TestGetSessionNotFound(t *testing.T) {
	_, ts := newTestServer(t)
	defer ts.Close()

	resp, err := http.Get(ts.URL + "/sessions/nonexistent")
	if err != nil {
		t.Fatalf("GET /sessions/nonexistent: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNotFound {
		t.Errorf("status = %d, want %d", resp.StatusCode, http.StatusNotFound)
	}
}

func TestSendTask(t *testing.T) {
	_, ts := newTestServer(t)
	defer ts.Close()

	// Create a session
	resp, err := http.Post(ts.URL+"/sessions", "application/json", nil)
	if err != nil {
		t.Fatalf("POST /sessions: %v", err)
	}
	var createResult map[string]interface{}
	if err := json.NewDecoder(resp.Body).Decode(&createResult); err != nil {
		t.Fatalf("decode create response: %v", err)
	}
	resp.Body.Close()

	sessionID := createResult["session_id"].(string)

	// Send task
	body := `{"task": "hello world"}`
	resp, err = http.Post(ts.URL+"/sessions/"+sessionID+"/task", "application/json", strings.NewReader(body))
	if err != nil {
		t.Fatalf("POST /sessions/%s/task: %v", sessionID, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		t.Errorf("status = %d, want %d", resp.StatusCode, http.StatusOK)
	}

	var result map[string]interface{}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		t.Fatalf("decode response: %v", err)
	}

	if result["status"] != "task_sent" {
		t.Errorf("status = %v, want task_sent", result["status"])
	}
}

func TestSendTaskEmptyBody(t *testing.T) {
	_, ts := newTestServer(t)
	defer ts.Close()

	// Create a session
	resp, err := http.Post(ts.URL+"/sessions", "application/json", nil)
	if err != nil {
		t.Fatalf("POST /sessions: %v", err)
	}
	var createResult map[string]interface{}
	if err := json.NewDecoder(resp.Body).Decode(&createResult); err != nil {
		t.Fatalf("decode create response: %v", err)
	}
	resp.Body.Close()

	sessionID := createResult["session_id"].(string)

	// Send empty task
	resp, err = http.Post(ts.URL+"/sessions/"+sessionID+"/task", "application/json", strings.NewReader(`{"task": ""}`))
	if err != nil {
		t.Fatalf("POST /sessions/%s/task: %v", sessionID, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusBadRequest {
		t.Errorf("status = %d, want %d", resp.StatusCode, http.StatusBadRequest)
	}
}

func TestCancelSession(t *testing.T) {
	_, ts := newTestServer(t)
	defer ts.Close()

	// Create a session
	resp, err := http.Post(ts.URL+"/sessions", "application/json", nil)
	if err != nil {
		t.Fatalf("POST /sessions: %v", err)
	}
	var createResult map[string]interface{}
	if err := json.NewDecoder(resp.Body).Decode(&createResult); err != nil {
		t.Fatalf("decode create response: %v", err)
	}
	resp.Body.Close()

	sessionID := createResult["session_id"].(string)

	// Cancel session
	req, err := http.NewRequest(http.MethodDelete, ts.URL+"/sessions/"+sessionID, nil)
	if err != nil {
		t.Fatalf("create DELETE request: %v", err)
	}
	resp, err = http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("DELETE /sessions/%s: %v", sessionID, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		t.Errorf("status = %d, want %d", resp.StatusCode, http.StatusOK)
	}
}

func TestStatusEndpoint(t *testing.T) {
	_, ts := newTestServer(t)
	defer ts.Close()

	resp, err := http.Get(ts.URL + "/status")
	if err != nil {
		t.Fatalf("GET /status: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		t.Errorf("status = %d, want %d", resp.StatusCode, http.StatusOK)
	}

	var result map[string]interface{}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		t.Fatalf("decode response: %v", err)
	}

	if _, ok := result["version"]; !ok {
		t.Error("response missing version")
	}
	if result["status"] != "running" {
		t.Errorf("status = %v, want running", result["status"])
	}
}

func TestCORSHeaders(t *testing.T) {
	_, ts := newTestServer(t)
	defer ts.Close()

	req, err := http.NewRequest(http.MethodOptions, ts.URL+"/status", nil)
	if err != nil {
		t.Fatalf("create OPTIONS request: %v", err)
	}
	req.Header.Set("Origin", "http://localhost:3000")

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("OPTIONS /status: %v", err)
	}
	defer resp.Body.Close()

	origin := resp.Header.Get("Access-Control-Allow-Origin")
	if origin != "*" {
		t.Errorf("Access-Control-Allow-Origin = %q, want %q", origin, "*")
	}
}
