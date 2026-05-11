package agent

import "github.com/woyin/OrangeCoding/modules/core"

// Compactor reduces conversation size by removing old messages when the token
// estimate exceeds a configured limit. It always preserves the system prompt
// and the last 5 messages.
type Compactor struct {
	maxTokens int
}

// NewCompactor creates a new Compactor with the given maximum token estimate.
func NewCompactor(maxTokens int) *Compactor {
	return &Compactor{maxTokens: maxTokens}
}

// Compact removes old non-system messages from the conversation until the token
// estimate is under the configured limit. The system prompt and the last 5
// messages are always preserved.
func (c *Compactor) Compact(conv *core.Conversation) error {
	estimate := conv.TokenEstimate()
	if estimate <= c.maxTokens {
		return nil
	}

	msgs := conv.Messages()
	if len(msgs) <= 6 { // system + 5 messages = nothing to compact
		return nil
	}

	// Identify system prompt and last 5 messages
	systemEnd := 0
	for i, m := range msgs {
		if m.Role == core.RoleSystem {
			systemEnd = i + 1
		} else {
			break
		}
	}

	keepFrom := len(msgs) - 5
	if keepFrom < systemEnd {
		keepFrom = systemEnd
	}

	// Keep system messages + middle messages that fit + last 5
	var newMsgs []core.Message
	newMsgs = append(newMsgs, msgs[:systemEnd]...)

	// Add messages between system and last-5, trimming from the front
	middle := msgs[systemEnd:keepFrom]
	middleStart := 0
	for {
		testMsgs := make([]core.Message, 0, len(newMsgs)+len(middle)-middleStart+5)
		testMsgs = append(testMsgs, newMsgs...)
		testMsgs = append(testMsgs, middle[middleStart:]...)
		testMsgs = append(testMsgs, msgs[keepFrom:]...)

		// Estimate tokens for the test set
		totalChars := 0
		for _, m := range testMsgs {
			totalChars += len(m.Content)
		}
		if totalChars/4 <= c.maxTokens || middleStart >= len(middle) {
			newMsgs = append(newMsgs, middle[middleStart:]...)
			break
		}
		middleStart++
	}

	newMsgs = append(newMsgs, msgs[keepFrom:]...)

	// Rebuild the conversation
	conv.Clear()
	for _, m := range newMsgs {
		conv.AddMessage(m)
	}

	return nil
}
