<?php

namespace App\MessageHandler;

use App\Message\DemoPingMessage;
use App\Repository\DemoJobRepository;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Component\Mercure\HubInterface;
use Symfony\Component\Mercure\Update;
use Symfony\Component\Messenger\Attribute\AsMessageHandler;
use Symfony\Contracts\Translation\TranslatorInterface;

#[AsMessageHandler]
final class DemoPingMessageHandler
{
    /**
     * Last greeting key handed out by this process, so two consecutive jobs
     * don't draw the same sentence (which reads like a bug). Persists across
     * jobs in the long-lived async worker; resets per request in sync mode,
     * where consecutive draws are independent anyway.
     */
    private ?string $lastKey = null;

    public function __construct(
        private readonly HubInterface $hub,
        private readonly EntityManagerInterface $em,
        private readonly DemoJobRepository $jobs,
        private readonly TranslatorInterface $translator,
    ) {
    }

    public function __invoke(DemoPingMessage $message): void
    {
        // Simulate a bit of background work (image processing, an API call…).
        usleep(700_000);

        $job = $this->jobs->find($message->jobId);
        if (null === $job) {
            return;
        }

        // Pick a random greeting key (never the same as the previous one) and
        // translate it in the requester's locale, injecting their name. Locale
        // comes from the message (no request here).
        $choices = array_values(array_filter(
            DemoPingMessage::GREETINGS,
            fn (string $k): bool => $k !== $this->lastKey,
        ));
        $key = $choices[array_rand($choices)];
        $this->lastKey = $key;
        $sentence = $this->translator->trans(
            $key,
            ['%username%' => $message->username],
            'messages',
            $message->locale,
        );

        // This write happens in the worker process (async) or inline in the
        // request (sync transport) — same handler either way.
        $job->setGeneratedSentence($sentence);
        $job->markCompleted();
        $this->em->flush();

        $this->hub->publish(new Update(
            DemoPingMessage::TOPIC,
            json_encode([
                'jobId' => $message->jobId,
                'status' => 'done',
                'sentence' => $sentence,
                'emittedAt' => (new \DateTimeImmutable())->format(DATE_ATOM),
            ], JSON_THROW_ON_ERROR),
            id: $message->jobId,
        ));
    }
}
