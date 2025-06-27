const { Keyring } = require('@polkadot/keyring');
const { mnemonicGenerate, cryptoWaitReady } = require('@polkadot/util-crypto');
const fs = require('fs').promises;
const path = require('path');
const inquirer = require('inquirer');
const config = require('../config/config');

async function createWallet(options) {
  try {
    await cryptoWaitReady();
    
    // Ensure wallets directory exists
    await fs.mkdir(config.walletsDir, { recursive: true });
    
    const keyring = new Keyring({ type: 'sr25519', ss58Format: config.ss58Format });
    
    // Generate mnemonic
    const mnemonic = mnemonicGenerate();
    
    // Create keypair from mnemonic
    const pair = keyring.addFromMnemonic(mnemonic);
    
    // Get wallet name
    let walletName = options.name;
    if (!walletName) {
      const answers = await inquirer.prompt([
        {
          type: 'input',
          name: 'name',
          message: 'Enter wallet name:',
          validate: (input) => input.length > 0 || 'Wallet name is required'
        }
      ]);
      walletName = answers.name;
    }
    
    // Prompt for password
    const { password } = await inquirer.prompt([
      {
        type: 'password',
        name: 'password',
        message: 'Enter password for wallet encryption:',
        mask: '*'
      }
    ]);
    
    // Create wallet data
    const walletData = {
      name: walletName,
      address: pair.address,
      publicKey: pair.publicKey,
      mnemonic: mnemonic,
      encoded: pair.toJson(password),
      created: new Date().toISOString()
    };
    
    // Save wallet
    const walletPath = path.join(config.walletsDir, `${walletName}.json`);
    await fs.writeFile(walletPath, JSON.stringify(walletData, null, 2));
    
    console.log('\n✅ Wallet created successfully!');
    console.log(`Name: ${walletName}`);
    console.log(`Address: ${pair.address}`);
    console.log(`File: ${walletPath}`);
    console.log('\n🔐 IMPORTANT: Save your mnemonic phrase securely!');
    console.log(`Mnemonic: ${mnemonic}`);
    console.log('\n⚠️  This mnemonic will not be shown again!');
    
  } catch (error) {
    console.error('❌ Error creating wallet:', error.message);
    process.exit(1);
  }
}

module.exports = { createWallet };