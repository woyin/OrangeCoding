package controlserver

import (
	"encoding/json"
	"log/slog"
	"net/http"
	"strings"

	"github.com/gin-gonic/gin"
	"github.com/gorilla/websocket"

	"github.com/woyin/OrangeCoding/modules/control-protocol"
)

var upgrader = websocket.Upgrader{
	CheckOrigin: func(r *http.Request) bool {
		origin := r.Header.Get("Origin")
		if origin == "" {
			return true // non-browser clients
		}
		// Allow same-origin and localhost for development.
		host := r.Host
		return strings.HasPrefix(origin, "http://"+host) ||
			strings.HasPrefix(origin, "https://"+host) ||
			strings.HasPrefix(origin, "http://localhost") ||
			strings.HasPrefix(origin, "http://127.0.0.1")
	},
}

// handleWebSocket upgrades HTTP to WebSocket and streams ServerEvent messages.
// Each client gets its own event subscription to avoid losing events.
func (s *Server) handleWebSocket(c *gin.Context) {
	conn, err := upgrader.Upgrade(c.Writer, c.Request, nil)
	if err != nil {
		slog.Error("websocket upgrade failed", "error", err)
		return
	}
	defer conn.Close()

	// Create a per-client event channel with buffering.
	clientCh := make(chan controlprotocol.ServerEvent, 32)
	unsub := s.subscribeEvents(clientCh)
	defer unsub()

	for {
		select {
		case event, ok := <-clientCh:
			if !ok {
				return
			}
			if err := writeEvent(conn, event); err != nil {
				slog.Debug("websocket write failed, client disconnected", "error", err)
				return
			}
		case <-c.Request.Context().Done():
			return
		}
	}
}

// writeEvent serializes a ServerEvent and sends it as a JSON WebSocket message.
func writeEvent(conn *websocket.Conn, event controlprotocol.ServerEvent) error {
	data, err := json.Marshal(event)
	if err != nil {
		return err
	}

	return conn.WriteMessage(websocket.TextMessage, data)
}
