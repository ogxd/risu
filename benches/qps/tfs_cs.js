import grpc from 'k6/net/grpc';
import { check, sleep } from 'k6';

const client = new grpc.Client();
client.load(['definitions'], 'prediction_service.proto');

export const options = {
    discardResponseBodies: true,
  
    scenarios: {
      contacts: {
        executor: 'ramping-arrival-rate',
  
        // Start iterations per `timeUnit`
        startRate: 5000,
  
        // Start `startRate` iterations per minute
        timeUnit: '1s',
  
        // Pre-allocate necessary VUs.
        preAllocatedVUs: 100,
  
        stages: [
          { target: 5000, duration: '60s' },
        ],
      },
    },
  };

export default () => {

  if (__ITER == 0) {
    // Only establish connection on the first iteration
    client.connect('127.0.0.1:8500', {
      plaintext: true
    });
  }

  const data = { "modelSpec": { "name": "call_limiter_alpha" }, "inputs": { "environmentType": { "dtype": "DT_INT32", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "intVal": [ 0 ] }, "hasIfa": { "dtype": "DT_INT32", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "intVal": [ 0 ] }, "isInterstitial": { "dtype": "DT_INT32", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "intVal": [ 0 ] }, "isHeaderBidding": { "dtype": "DT_INT32", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "intVal": [ 0 ] }, "serverSideBiddingCallerId": { "dtype": "DT_INT32", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "intVal": [ 0 ] }, "outputDomain": { "dtype": "DT_STRING", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "stringVal": [ "" ] }, "hasEids": { "dtype": "DT_INT32", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "intVal": [ 0 ] }, "floorConstraint": { "dtype": "DT_FLOAT", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "floatVal": [ 0 ] }, "dspPartnerId": { "dtype": "DT_INT32", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "intVal": [ 0 ] }, "networkId": { "dtype": "DT_INT32", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "intVal": [ 0 ] }, "quovaCountryId": { "dtype": "DT_INT32", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "intVal": [ 0 ] }, "hasBuyerUid": { "dtype": "DT_INT32", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "intVal": [ 0 ] }, "outputBundleId": { "dtype": "DT_STRING", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "stringVal": [ "" ] }, "impressionType": { "dtype": "DT_INT32", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "intVal": [ 0 ] }, "platform": { "dtype": "DT_INT32", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "intVal": [ 0 ] }, "osId": { "dtype": "DT_INT32", "tensorShape": { "dim": [ { "size": "1" }, { "size": "1" } ] }, "intVal": [ 0 ] } } }
  ;

  const response = client.invoke('tensorflow.serving.PredictionService/Predict', data);

  check(response, {
    'status is OK': (r) => r && r.status === grpc.StatusOK,
  });

  //console.log(JSON.stringify(response.message));

  //client.close();
  //sleep(1);
};
