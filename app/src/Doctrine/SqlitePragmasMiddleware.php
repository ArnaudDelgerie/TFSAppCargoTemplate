<?php

namespace App\Doctrine;

use Doctrine\DBAL\Driver;
use Doctrine\DBAL\Driver\Connection as DriverConnection;
use Doctrine\DBAL\Driver\Middleware;
use Doctrine\DBAL\Driver\Middleware\AbstractDriverMiddleware;

/**
 * Applies the SQLite PRAGMAs the app relies on for safe multi-process access
 * (the HTTP server and the Messenger worker share the same file).
 *
 * DBAL 4 removed the postConnect event system, so the supported way to run
 * statements on every new connection is a driver middleware. Auto-tagged with
 * `doctrine.middleware` by DoctrineBundle via the Middleware interface.
 */
final class SqlitePragmasMiddleware implements Middleware
{
    public function wrap(Driver $driver): Driver
    {
        return new class($driver) extends AbstractDriverMiddleware {
            public function connect(array $params): DriverConnection
            {
                $connection = parent::connect($params);

                // Only meaningful for SQLite; skip any other platform.
                if (str_contains($params['driver'] ?? '', 'sqlite')) {
                    $connection->exec('PRAGMA busy_timeout = 5000');
                    $connection->exec('PRAGMA journal_mode = WAL');
                    $connection->exec('PRAGMA synchronous = NORMAL');
                }

                return $connection;
            }
        };
    }
}
