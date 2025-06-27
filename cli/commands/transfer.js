const { getApi, closeApi } = require('../utils/api');
const { Keyring } = require('@polkadot/keyring');
const { cryptoWaitReady } = require('@polkadot/util-crypto');
const fs = require('fs').promises;
const inquirer = require('inquirer');

async function transfer(options) {
  try {
    await cryptoWaitReady();
    const api = await getApi();
    const keyring = new Keyring({ type: 'sr25519' });
    
    // Load sender account
    const walletData = JSON.parse(await fs.readFile(options.from, 'utf8'));
    
    const { password } = await inquirer.prompt([
      {
        type: 'password',
        name: 'password',
        message: 'Enter wallet password:',
        mask: '*'
      }
    ]);
    
    const senderAccount = keyring.addFromJson(walletData.encoded);
    senderAccount.decodePkcs8(password);
    
    console.log(`\n💸 Initiating transfer...`);
    console.log(`From: ${senderAccount.address}`);
    console.log(`To: ${options.to}`);
    console.log(`Amount: ${options.amount}`);
    
    const transfer = api.tx.balances.transferKeepAlive(options.to, options.amount);
    
    // Sign and send transaction
    const hash = await transfer.signAndSend(senderAccount, (result) => {
      if (result.status.isInBlock) {
        console.log(`✅ Transaction included in block: ${result.status.asInBlock}`);
      } else if (result.status.isFinalized) {
        console.log(`✅ Transaction finalized: ${result.status.asFinalized}`);
        closeApi();
      }
    });    
  } catch (error) {
    console.error('❌ Error during transfer:', error.message);
    await closeApi();
  }
}

module.exports = { transfer };