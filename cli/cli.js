#!/usr/bin/env node

const { Command } = require('commander');
const { createWallet } = require('./commands/wallet');
const { airdrop } = require('./commands/airdrop');
const { transfer } = require('./commands/transfer');
const { getBalance } = require('./commands/balance');

const program = new Command();

program
  .name('substrate-cli')
  .description('CLI for Substrate chain operations')
  .version('1.0.0');

program
  .command('create-wallet')
  .description('Create a new wallet')
  .option('-n, --name <name>', 'wallet name')
  .action(createWallet);

program
  .command('airdrop')
  .description('Airdrop tokens to an address')
  .requiredOption('-t, --to <address>', 'recipient address')
  .requiredOption('-a, --amount <amount>', 'amount to airdrop')
  .option('-s, --sudo <keyfile>', 'sudo account keyfile')
  .action(airdrop);

program
  .command('transfer')
  .description('Transfer tokens between accounts')
  .requiredOption('-f, --from <keyfile>', 'sender keyfile')
  .requiredOption('-t, --to <address>', 'recipient address')
  .requiredOption('-a, --amount <amount>', 'amount to transfer')
  .action(transfer);

program
  .command('balance')
  .description('Check account balance')
  .requiredOption('-a, --address <address>', 'account address')
  .action(getBalance);

program.parse();