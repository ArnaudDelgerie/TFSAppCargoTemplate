<?php

namespace App\MessageHandler;

use App\Message\DemoPingMessage;
use App\Repository\DemoJobRepository;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Component\Mercure\HubInterface;
use Symfony\Component\Mercure\Update;
use Symfony\Component\Messenger\Attribute\AsMessageHandler;

#[AsMessageHandler]
final readonly class DemoPingMessageHandler
{
    public function __construct(
        private HubInterface $hub,
        private EntityManagerInterface $em,
        private DemoJobRepository $jobs,
    ) {
    }

    public function __invoke(DemoPingMessage $message): void
    {
        usleep(700_000);

        // Mark the persisted job done — this write happens in the worker
        // process, exercising the shared SQLite DB across processes.
        $job = $this->jobs->find($message->jobId);
        if (null !== $job) {
            $job->markCompleted();
            $this->em->flush();
        }

        $this->hub->publish(new Update(
            DemoPingMessage::TOPIC,
            json_encode([
                'jobId' => $message->jobId,
                'status' => 'done',
                'emittedAt' => (new \DateTimeImmutable())->format(DATE_ATOM),
            ], JSON_THROW_ON_ERROR),
            id: $message->jobId,
        ));
    }
}
