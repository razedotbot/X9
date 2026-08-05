// Command fullsurface tours the swap surface a self-hosted raze-router serves.
//
// Nothing is submitted on-chain — the router returns unsigned transactions that
// you sign and send yourself.
//
//	go run ./examples/fullsurface
package main

import (
	"context"
	"fmt"
	"log"

	razetrading "raze.bot/trading"
)

func main() {
	// Your license key. It travels as X-Api-Key, which the router checks first.
	client := razetrading.New("rzl_your_license_key",
		razetrading.WithBaseURL("http://localhost:8082"))
	ctx := context.Background()

	health, err := client.Health(ctx)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println("health", health.Status, "slot", health.Slot)

	// Exact-in quote: 1 SOL into the mint, 0.5% slippage.
	amount := uint64(1_000_000_000)
	slippage := 50
	quote, err := client.Quote(ctx, "TokenMintPubkey", &razetrading.QuoteParams{
		Amount:      &amount,
		SlippageBps: &slippage,
	})
	if err != nil {
		log.Fatal(err)
	}
	if quote.Data != nil {
		fmt.Println("quote out", quote.Data.AmountOut, "via", quote.Data.Platform)
	}

	// Buy. Send the key: an unauthenticated call falls back to the compiled
	// public fee tier, whose recipient is not yours.
	solAmount := 0.1
	buy, err := client.Buy(ctx, razetrading.BuyOpts{
		WalletAddresses: []string{"YourWalletPubkey"},
		TokenAddress:    "TokenMintPubkey",
		SolAmount:       &solAmount,
	})
	if err != nil {
		log.Fatal(err)
	}
	// Entries are index-aligned with WalletAddresses; a wallet the router could
	// not build for comes back as an empty string, so skip those.
	built := 0
	for _, tx := range buy.Transactions {
		if tx != "" {
			built++
		}
	}
	fmt.Println("buy txs", built, "of", len(buy.Transactions))

	// Sell the whole position.
	pct := 100.0
	sell, err := client.Sell(ctx, razetrading.SellOpts{
		WalletAddresses: []string{"YourWalletPubkey"},
		TokenAddress:    "TokenMintPubkey",
		Percentage:      &pct,
	})
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println("sell ok", sell.Success, "entries", len(sell.Transactions))
}
