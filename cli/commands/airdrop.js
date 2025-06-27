const { getApi, closeApi } = require('../utils/api');
const { Keyring } = require('@polkadot/keyring');
const { cryptoWaitReady } = require('@polkadot/util-crypto');
const fs = require('fs').promises;
const inquirer = require('inquirer');
const config = require('../config/config');

async function airdrop(options) {
  try {
    await cryptoWaitReady();
    const api = await getApi();
    const keyring = new Keyring({ type: 'sr25519' });
    
    let sudoAccount;
    
    if (options.sudo) {
      // Load sudo account from file
      const walletData = JSON.parse(await fs.readFile(options.sudo, 'utf8'));
      
      const { password } = await inquirer.prompt([
        {
          type: 'password',
          name: 'password',
          message: 'Enter wallet password:',
          mask: '*'
        }
      ]);
      
      sudoAccount = keyring.addFromJson(walletData.encoded);
      sudoAccount.decodePkcs8(password);
    } else {
      // Use Alice for development (remove in production)
      sudoAccount = keyring.addFromUri('//Alice');
      console.log('⚠️  Using Alice account for airdrop (development only)');
    }
    
    const amount = options.amount;
    
    console.log(`\n🚁 Initiating airdrop...`);
    console.log(`To: ${options.to}`);
    console.log(`Amount: ${amount}`);
    // Create and send the airdrop transaction using sudo
    const transfer = api.tx.balances.forceSetBalance(options.to, amount);
    const sudoTx = api.tx.sudo.sudo(transfer);
    
    const hash = await sudoTx.signAndSend(sudoAccount);
    
    console.log(`✅ Airdrop transaction sent!`);
    console.log(`Transaction hash: ${hash.toHex()}`);
    
  } catch (error) {
    console.error('❌ Error during airdrop:', error.message);
  } finally {
    await closeApi();
  }
}

module.exports = { airdrop };