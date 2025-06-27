const { getApi, closeApi } = require('../utils/api');

async function getBalance(options) {
  try {
    const api = await getApi();
    
    const { data: balance } = await api.query.system.account(options.address);
    
    console.log(`\n💰 Balance for ${options.address}:`);
    console.log(`Free: ${balance.free.toHuman()}`);
    console.log(`Reserved: ${balance.reserved.toHuman()}`);
    console.log(`Frozen: ${balance.frozen.toHuman()}`);
    
  } catch (error) {
    console.error('❌ Error fetching balance:', error.message);
  } finally {
    await closeApi();
  }
}

module.exports = { getBalance };