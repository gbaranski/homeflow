package mqtt

import (
	"context"
	"crypto/ed25519"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"log"
	"os"
	"strings"
	"time"

	"github.com/gbaranski/houseflow/pkg/types"
	"github.com/gbaranski/houseflow/pkg/utils"

	paho "github.com/eclipse/paho.mqtt.golang"
)

type MQTTOptions struct {
	// ClientID, required
	ClientID string

	// Default: "tcp://emqx:1883/mqtt"
	BrokerURL string

	// KeepAlive
	//
	// Default: 30s
	KeepAlive time.Duration

	// PingTimeout
	//
	// Default: 5s
	PingTimeout time.Duration

  // PrivateKey is servers private key
  PrivateKey ed25519.PrivateKey
}

// Parses options to the defaults
func (opts *MQTTOptions) Parse() {
	if opts.BrokerURL == "" {
		opts.BrokerURL = "tcp://emqx:1883/mqtt"
	}
	if opts.KeepAlive == 0 {
		opts.KeepAlive = time.Second * 30
	}

	if opts.PingTimeout == 0 {
		opts.PingTimeout = time.Second * 5
	}
}

type MQTT struct {
	client paho.Client
	opts   MQTTOptions
}

func NewMQTT(opts MQTTOptions) MQTT {
	paho.ERROR = log.New(os.Stdout, "[ERROR] ", 0)
	paho.CRITICAL = log.New(os.Stdout, "[CRIT] ", 0)
	paho.WARN = log.New(os.Stdout, "[WARN]  ", 0)
	// mqtt.DEBUG = log.New(os.Stdout, "[DEBUG] ", 0)

	// Add there some kind of password soon
	copts := paho.
		NewClientOptions().
		AddBroker(opts.BrokerURL).
		SetClientID(opts.ClientID).
		SetKeepAlive(opts.KeepAlive).
		SetPingTimeout(opts.PingTimeout)

	c := paho.NewClient(copts)
	if token := c.Connect(); token.Wait() && token.Error() != nil {
		panic(token.Error())
	}
	return MQTT{
		client: c,
		opts:   opts,
	}
}

var ErrDeviceTimeout = errors.New("device timeout")
var ErrInvalidSignature = errors.New("invalid signature")

func (m *MQTT) SendRequestWithResponse(ctx context.Context, device types.Device, req types.DeviceRequest) (*types.DeviceResponse, error) {
	reqTopic := fmt.Sprintf("%s/command/request", device.ID.Hex())
	resTopic := fmt.Sprintf("%s/command/response", device.ID.Hex())
	msgc := make(chan paho.Message)

	if token := m.client.Subscribe(resTopic, 0, func(c paho.Client, m paho.Message) {
		msgc <- m
	}); token.Wait() && token.Error() != nil {
		return nil, token.Error()
	}

	defer func() {
		m.client.Unsubscribe(resTopic)
	}()

	reqjson, err := json.Marshal(req)
	if err != nil {
		return nil, err
	}

	ssig := ed25519.Sign(m.opts.PrivateKey, reqjson)
	encssig := base64.StdEncoding.EncodeToString(ssig)

	reqp := strings.Join([]string{encssig, string(reqjson)}, ".")

	if token := m.client.Publish(reqTopic, 0, false, reqp); token.Wait() && token.Error() != nil {
		return nil, token.Error()
	}

readMessages:
	for {
		select {
		case <-ctx.Done():
			return nil, ErrDeviceTimeout

		case msg, ok := <-msgc:
			fmt.Println("Received some message")
			if !ok {
				fmt.Println("Failed waiting for msg for unknown reason")
				continue readMessages
			}

			resp := msg.Payload()
			resjson, dsig, err := utils.ParseSignedPayload(resp)
			if err != nil {
				fmt.Println("Failed parsing payload to json and sig: ", err.Error())
				continue readMessages
			}
			var res types.DeviceResponse
			err = json.Unmarshal([]byte(resjson), &res)
			if err != nil {
				fmt.Println("Failed unmarshalling json: ", err.Error())
				continue readMessages
			}
			if res.CorrelationData != req.CorrelationData {
				fmt.Println("Correlation data doesn't match, skipping")
				continue readMessages
			}
			// TODO: make database store raw bin
			dpkey, err := base64.StdEncoding.DecodeString(device.PublicKey)
			if err != nil {
				fmt.Println("fail parsing device public key: ", err.Error())
				return nil, fmt.Errorf("fail parsing device public key %s", err.Error())
			}
			valid := ed25519.Verify(ed25519.PublicKey(dpkey), []byte(resjson), dsig)
			if !valid {
				return nil, ErrInvalidSignature
			}
			return &res, nil
		}
	}
}
