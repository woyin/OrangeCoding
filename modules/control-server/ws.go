package controlserver

import (
	"encoding/json"
	"log/slog"
	"net/http"

	"github.com/gin-gonic/gin"
	"github.com/gorilla/websocket"

	"github.com/woyin/OrangeCoding/modules/control-protocol"
)

var upgrader = websocket.Upgrader{
	CheckOrigin: func(r *http.Request) bool {
		return true // Allow all origins for development
	},
}

// handleWebSocket upgrades HTTP to WebSocket and streams ServerEvent messages.
func (s *Server) handleWebSocket(c *gin.Context) {
	conn, err := upgrader.Upgrade(c.Writer, c.Request, nil)
	if err != nil {
		slog.Error("websocket upgrade failed", "error", err)
		return
	}
	defer conn.Close()

	// Read events from the shared channel and forward to WebSocket.
	// Since the eventCh is shared across all clients, we consume events
	// and broadcast them to the connected client.
	for {
		select {
		case event, ok := <-s.eventCh:
			if !ok {
				// Channel closed
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
