package controlserver

import (
	"encoding/json"
	"net/http"

	"github.com/gin-gonic/gin"
	"github.com/google/uuid"

	"github.com/woyin/OrangeCoding/modules/control-protocol"
)

// createSession handles POST /sessions.
func (s *Server) createSession(c *gin.Context) {
	sessionID := uuid.New().String()

	err := s.workers.StartSession(sessionID, nil)
	if err != nil {
		c.JSON(http.StatusConflict, gin.H{"error": err.Error()})
		return
	}

	c.JSON(http.StatusOK, gin.H{
		"session_id": sessionID,
		"status":     "running",
	})
}

// listSessions handles GET /sessions.
func (s *Server) listSessions(c *gin.Context) {
	sessions := s.workers.ListSessions()

	type sessionInfo struct {
		SessionID string `json:"session_id"`
		Status    string `json:"status"`
	}

	result := make([]sessionInfo, 0, len(sessions))
	for _, id := range sessions {
		status, _ := s.workers.GetStatus(id)
		result = append(result, sessionInfo{
			SessionID: id,
			Status:    status,
		})
	}

	c.JSON(http.StatusOK, result)
}

// getSession handles GET /sessions/:id.
func (s *Server) getSession(c *gin.Context) {
	sessionID := c.Param("id")

	status, ok := s.workers.GetStatus(sessionID)
	if !ok {
		c.JSON(http.StatusNotFound, gin.H{"error": "session not found"})
		return
	}

	c.JSON(http.StatusOK, gin.H{
		"session_id": sessionID,
		"status":     status,
	})
}

// sendTask handles POST /sessions/:id/task.
func (s *Server) sendTask(c *gin.Context) {
	sessionID := c.Param("id")

	var body struct {
		Task string `json:"task"`
	}
	if err := c.ShouldBindJSON(&body); err != nil {
		// Try reading raw body for non-JSON
		rawBody, readErr := c.GetRawData()
		if readErr != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid request body"})
			return
		}

		// Re-attempt JSON parse from raw body
		if jsonErr := json.Unmarshal(rawBody, &body); jsonErr != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid JSON: " + jsonErr.Error()})
			return
		}
	}

	if body.Task == "" {
		c.JSON(http.StatusBadRequest, gin.H{"error": "task is required"})
		return
	}

	_, ok := s.workers.GetStatus(sessionID)
	if !ok {
		c.JSON(http.StatusNotFound, gin.H{"error": "session not found"})
		return
	}

	// Send task_update event through the event channel
	if s.eventCh != nil {
		s.eventCh <- &controlprotocol.TaskUpdateEvent{
			SessionID: sessionID,
			Status:    "task_received",
			Message:   body.Task,
		}
	}

	c.JSON(http.StatusOK, gin.H{
		"session_id": sessionID,
		"status":     "task_sent",
	})
}

// cancelSession handles DELETE /sessions/:id.
func (s *Server) cancelSession(c *gin.Context) {
	sessionID := c.Param("id")

	err := s.workers.StopSession(sessionID)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": err.Error()})
		return
	}

	c.JSON(http.StatusOK, gin.H{
		"session_id": sessionID,
		"status":     "cancelled",
	})
}

// status handles GET /status.
func (s *Server) status(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{
		"version": "0.1.0",
		"status":  "running",
	})
}
