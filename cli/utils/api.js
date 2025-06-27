const { ApiPromise, WsProvider } = require('@polkadot/api');
const config = require('../config/config');

let api = null;

async function getApi() {
  if (!api) {
    const provider = new WsProvider(config.wsEndpoint);
    api = await ApiPromise.create({ provider });
    console.log(`Connected to ${await api.rpc.system.chain()} chain`);
  }
  return api;
}

async function closeApi() {
  if (api) {
    await api.disconnect();
    api = null;
  }
}

module.exports = { getApi, closeApi };