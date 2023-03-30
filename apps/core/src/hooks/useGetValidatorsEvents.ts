// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { useRpcClient } from '../api/RpcClientContext';
import { type EventId, VALIDATORS_EVENTS_QUERY, PaginatedEvents, SuiEvent } from '@mysten/sui.js';
import { useQuery } from '@tanstack/react-query';

type GetValidatorsEvent = {
    cursor?: EventId | null;
    limit: number | null;
    order: 'ascending' | 'descending';
};

// NOTE: This copys the query limit from our Rust JSON RPC backend, this needs to be kept in sync!
const QUERY_MAX_RESULT_LIMIT = 1;

//TODO: get validatorEvents by validator address
export function useGetValidatorsEvents({
    cursor,
    limit,
    order,
}: GetValidatorsEvent) {
    const rpc = useRpcClient();
    // since we are getting events based on the number of validators, we need to make sure that the limit is not null and cache by the limit
    // number of validators can change from network to network
    return useQuery(
        ['validatorEvents', limit, cursor?.txDigest, order],
        async () => {
            if (limit === null) {
                // Do some validation at the runtime level for some extra type-safety
                // https://tkdodo.eu/blog/react-query-and-type-script#type-safety-with-the-enabled-option
                throw new Error(`Limit needs to always be defined! Received ${limit} instead.`)
            }

            if (limit > QUERY_MAX_RESULT_LIMIT) {
                let hasNextPage = true;
                let currCursor = cursor;
                const results = [];
    
                while (hasNextPage) {
                    console.log("REQUESTING WITH CURSOR", currCursor)
                    const validatorEventsResponse = await rpc.queryEvents({
                        query: { MoveEventType: VALIDATORS_EVENTS_QUERY },
                        cursor: currCursor ? {
                            atCheckpoint: currCursor.eventSeq,
                            objectId: currCursor.txDigest,
                        } : null,
                        limit: Math.min(limit, QUERY_MAX_RESULT_LIMIT),
                        order,
                    });

                    console.log("GOT RESULTS", validatorEventsResponse)
    
                    hasNextPage = validatorEventsResponse.hasNextPage;
                    currCursor = validatorEventsResponse.nextCursor;
                    // console.log("ADDING RESULTS");
                    // console.log('set', validatorEventsResponse.data);
                    results.push(...validatorEventsResponse.data);
                }

                console.log(results);

                return results;
            }

            const validatorEventsResponse = await rpc.queryEvents({
                query: { MoveEventType: VALIDATORS_EVENTS_QUERY },
                cursor: cursor?.txDigest,
                limit,
                order,
            });

            console.log("NON PAGE", validatorEventsResponse)
            return validatorEventsResponse.data;
        },
        { enabled: !!limit }
    );
}
