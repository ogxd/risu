import grpc from 'k6/net/grpc';
import { check, sleep } from 'k6';

const client = new grpc.Client();
client.load(['definitions'], 'hello.proto');

export const options = {
    discardResponseBodies: true,
  
    scenarios: {
      contacts: {
        executor: 'ramping-arrival-rate',
  
        // Start iterations per `timeUnit`
        startRate: 1000,
  
        // Start `startRate` iterations per minute
        timeUnit: '1s',
  
        // Pre-allocate necessary VUs.
        preAllocatedVUs: 100,
  
        stages: [
          { target: 50000, duration: '60s' },
        ],
      },
    },
  };

export default () => {

  if (__ITER == 0) {
    // Only establish connection on the first iteration
    client.connect('127.0.0.1:3001', {
      plaintext: true
    });
  }

  const data = { name: 'Bert' };
  const response = client.invoke('helloworld.Greeter/SayHello', data);

  check(response, {
    'status is OK': (r) => r && r.status === grpc.StatusOK,
  });

  //console.log(JSON.stringify(response.message));

  //client.close();
  //sleep(1);
};
